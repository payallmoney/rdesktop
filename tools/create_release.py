# -*- coding: utf-8 -*-
"""创建 vX.Y.Z GitHub Release 并上传 exe(AGENTS.md 发版规则的标准流程)。
用法:python tools/create_release.py 1.1.0 [更新说明]
不打印令牌;直连绕过代理。"""
import json, os, subprocess, sys, urllib.request

REPO = 'payallmoney/rdesktop'

def cred():
    inp = 'protocol=https\nhost=github.com\n\n'.encode()
    p = subprocess.run(['git', 'credential', 'fill'], input=inp, capture_output=True)
    for ln in p.stdout.decode('utf-8', 'replace').splitlines():
        if ln.startswith('password='):
            return ln.split('=', 1)[1]
    return None

opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))

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
        return e.code, e.read().decode('utf-8', 'replace')[:400]

def main():
    if len(sys.argv) < 2:
        print('usage: create_release.py X.Y.Z [notes]'); sys.exit(1)
    ver = sys.argv[1].lstrip('v')
    notes = sys.argv[2] if len(sys.argv) > 2 else 'rdesktop v%s' % ver
    exe = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
                       'target', 'release', 'rdesktop.exe')
    tok = cred()
    if not tok:
        print('NO_CREDENTIALS'); sys.exit(2)

    tag = 'v' + ver
    st, rel = api('GET', f'https://api.github.com/repos/{REPO}/releases/tags/{tag}', tok)
    if st == 200:
        print('release exists, reuse id', rel['id'])
    else:
        st, rel = api('POST', f'https://api.github.com/repos/{REPO}/releases', tok,
                      data=json.dumps({
                          'tag_name': tag,
                          'name': 'rdesktop ' + tag,
                          'body': notes,
                          'draft': False,
                          'prerelease': False,
                      }).encode())
        if st != 201:
            print('CREATE_FAIL', st, str(rel)[:300]); sys.exit(3)
        print('release created:', tag)

    name = f'rdesktop-{tag}-win-x64.exe'
    old = next((a for a in rel.get('assets', []) if a['name'] == name), None)
    if old:
        st, _ = api('DELETE', old['url'], tok)
        print('delete old asset:', st)
    data = open(exe, 'rb').read()
    st, up = api('POST',
                 f"https://uploads.github.com/repos/{REPO}/releases/{rel['id']}/assets?name={name}",
                 tok, data=data, ctype='application/octet-stream')
    if st == 201:
        print('UPLOADED', up['name'], up['size'], 'bytes')
    else:
        print('UPLOAD_FAIL', st, str(up)[:300]); sys.exit(4)

if __name__ == '__main__':
    main()
