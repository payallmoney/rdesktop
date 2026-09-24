# -*- coding: utf-8 -*-
"""发布到 GitHub:用凭据管理器中的 GitHub PAT 建仓、推送、创建 v1.0.0 Release 并上传 exe。
绝不打印令牌。"""
import json
import os
import subprocess
import sys
import urllib.request

REPO_NAME = 'rdesktop'
VERSION = 'v1.0.0'
EXE = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), 'target', 'release', 'rdesktop.exe')

def cred_fill():
    inp = 'protocol=https\nhost=github.com\n\n'.encode()
    p = subprocess.run(['git', 'credential', 'fill'], input=inp, capture_output=True)
    out = p.stdout.decode('utf-8', 'replace')
    user = pwd = None
    for ln in out.splitlines():
        if ln.startswith('username='):
            user = ln.split('=', 1)[1]
        elif ln.startswith('password='):
            pwd = ln.split('=', 1)[1]
    return user, pwd

def api(method, path, token, data=None, raw=None, ctype='application/json'):
    url = path if path.startswith('https://') else 'https://api.github.com' + path
    req = urllib.request.Request(url, method=method)
    req.add_header('Authorization', 'token ' + token)
    req.add_header('User-Agent', 'rdesktop-publish')
    req.add_header('Accept', 'application/vnd.github+json')
    body = None
    if data is not None:
        body = json.dumps(data).encode()
        req.add_header('Content-Type', 'application/json')
    if raw is not None:
        body = raw
        req.add_header('Content-Type', ctype)
    try:
        with urllib.request.urlopen(req, body) as resp:
            txt = resp.read().decode('utf-8', 'replace')
            return resp.status, (json.loads(txt) if txt.strip().startswith(('{', '[')) else txt)
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode('utf-8', 'replace')[:400]

user, pwd = cred_fill()
if not pwd:
    print('NO_CREDENTIALS: 凭据管理器中没有 github.com 的凭证')
    sys.exit(2)
print('credential found for user:', user)

# 校验令牌
st, me = api('GET', '/user', pwd)
if st != 200:
    print('TOKEN_INVALID:', st, str(me)[:200])
    sys.exit(3)
login = me['login']
print('authenticated as:', login)

# 建仓库(公开)
st, repo = api('POST', '/user/repos', pwd, {
    'name': REPO_NAME,
    'description': 'Windows 桌面分组管理工具:半透明圆角面板接管桌面图标(Rust + Win32 API)',
    'homepage': '',
    'private': False,
    'has_issues': True, 'has_wiki': False, 'auto_init': False,
})
if st == 201:
    print('repo created')
elif st == 422 and 'already exists' in str(repo):
    print('repo already exists')
else:
    print('REPO_CREATE_FAIL:', st, str(repo)[:300])
    sys.exit(4)

# 推送
remote = f'https://github.com/{login}/{REPO_NAME}.git'
subprocess.run(['git', 'remote', 'remove', 'origin'], capture_output=True)
r = subprocess.run(['git', 'remote', 'add', 'origin', remote])
r = subprocess.run(['git', 'push', '-u', 'origin', 'master'], capture_output=True)
out = (r.stdout + r.stderr).decode('utf-8', 'replace')
print('push rc=', r.returncode)
print(out[-600:])
if r.returncode != 0:
    sys.exit(5)

# 打 tag + 推送
subprocess.run(['git', 'tag', '-f', VERSION])
r = subprocess.run(['git', 'push', 'origin', VERSION], capture_output=True)
print('tag push rc=', r.returncode, (r.stdout + r.stderr).decode('utf-8', 'replace')[-200:])

# 创建 Release
st, rel = api('POST', f'/repos/{login}/{REPO_NAME}/releases', pwd, {
    'tag_name': VERSION,
    'name': 'rdesktop ' + VERSION,
    'body': ('Windows 桌面分组管理工具首个正式版。\n\n'
             '## 亮点\n'
             '- 半透明圆角分组面板接管桌面,面板间自由拖拽、框选与 Ctrl/Shift 多选\n'
             '- 面板高度不足自动出现垂直滚动条\n'
             '- 单面板 图层/隐藏/标题显示/重命名/删除,设置对话框批量管理\n'
             '- 图标右键 = 资源管理器同款原生菜单\n'
             '- 状态全量持久化(注册表),支持开机自启动\n'
             '- Win+D 快速切换、拖拽/缩放稳定性硬化\n\n'
             '详见 README。'),
    'draft': False,
    'prerelease': False,
})
if st != 201:
    print('RELEASE_FAIL:', st, str(rel)[:300])
    sys.exit(6)
upload_url = rel['upload_url'].split('{')[0]
print('release created:', rel.get('html_url', ''))

# 上传 exe
with open(EXE, 'rb') as f:
    exe_bytes = f.read()
st, asset = api('POST', upload_url + '?name=rdesktop-%s-win-x64.exe' % VERSION, pwd,
                raw=exe_bytes, ctype='application/octet-stream')
print('asset upload:', st, asset.get('state') if isinstance(asset, dict) else str(asset)[:200])
print('DONE:', rel.get('html_url', ''))
