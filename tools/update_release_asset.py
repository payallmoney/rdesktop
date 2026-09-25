# -*- coding: utf-8 -*-
"""更新 v1.0.0 release 的 exe 资产为最新构建。用凭据管理器中的 GitHub PAT,不打印令牌。"""
import json, os, subprocess, sys, urllib.request

REPO = 'payallmoney/rdesktop'
TAG = 'v1.0.0'
EXE = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
                   'target', 'release', 'rdesktop.exe')
NAME = 'rdesktop-v1.0.0-win-x64.exe'

# 直连,绕过不稳定代理
opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))

def cred():
    inp = 'protocol=https\nhost=github.com\n\n'.encode()
    p = subprocess.run(['git', 'credential', 'fill'], input=inp, capture_output=True)
    for ln in p.stdout.decode('utf-8', 'replace').splitlines():
        if ln.startswith('password='):
            return ln.split('=', 1)[1]
    return None

def api(method, url, token, data=None, ctype='application/json'):
    req = urllib.request.Request(url, method=method)
    req.add_header('Authorization', 'token ' + token)
    req.add_header('User-Agent', 'rdesktop-publish')
    req.add_header('Accept', 'application/vnd.github+json')
    body = None
    if data is not None:
        body = data
        req.add_header('Content-Type', ctype)
    try:
        with opener.open(req, body) as resp:
            txt = resp.read().decode('utf-8', 'replace')
            return resp.status, (json.loads(txt) if txt.strip().startswith(('{', '[')) else txt)
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode('utf-8', 'replace')[:300]

tok = cred()
if not tok:
    print('NO_CREDENTIALS'); sys.exit(2)

st, rel = api('GET', f'https://api.github.com/repos/{REPO}/releases/tags/{TAG}', tok)
if st != 200:
    print('RELEASE_GET_FAIL', st); sys.exit(3)
old = next((a for a in rel['assets'] if a['name'] == NAME), None)
if old:
    st, _ = api('DELETE', old['url'], tok)
    print('delete old asset:', st)

data = open(EXE, 'rb').read()
st, up = api('POST', f"https://uploads.github.com/repos/{REPO}/releases/{rel['id']}/assets?name={NAME}",
             tok, data=data, ctype='application/octet-stream')
if st == 201:
    print('UPLOADED', up['name'], up['size'], 'bytes')
else:
    print('UPLOAD_FAIL', st, str(up)[:300]); sys.exit(4)
