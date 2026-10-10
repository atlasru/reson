"""Validate the release Tauri desktop, using real IPC, SoundCloud and libmpv.

Run tauri-driver first. No browser server, injected tracks or mock backend.
Screenshots are captured from the native application's WebView.
"""
import argparse
import base64
import json
import os
import pathlib
import time
import urllib.error
import urllib.request

parser = argparse.ArgumentParser()
parser.add_argument('--application', required=True)
parser.add_argument('--driver-url', default='http://127.0.0.1:4444')
parser.add_argument('--output', default='.smoke/redesign')
parser.add_argument('--profile', default='scottbuckley')
args = parser.parse_args()
output = pathlib.Path(args.output)
output.mkdir(parents=True, exist_ok=True)
http = urllib.request.build_opener(urllib.request.ProxyHandler({}))
base = args.driver_url.rstrip('/')
root = None
checks = []

def request(path, data=None, method=None):
    body = None if data is None else json.dumps(data).encode()
    try:
        with http.open(urllib.request.Request(base + path, data=body, method=method, headers={'Content-Type': 'application/json'}), timeout=240) as response:
            result = json.load(response)
    except urllib.error.HTTPError as error:
        raise RuntimeError(error.read().decode()) from error
    value = result.get('value')
    if isinstance(value, dict) and 'error' in value and 'ok' not in value:
        raise RuntimeError(value)
    return value

def js(code):
    return request(root + '/execute/sync', {'script': code, 'args': []})

def rpc(name, values=None):
    result = request(root + '/execute/async', {'script': 'const done=arguments[arguments.length-1];window.__TAURI_INTERNALS__.invoke(' + json.dumps(name) + ',' + json.dumps(values or {}) + ').then(value=>done({ok:true,value}),error=>done({ok:false,error:String(error)}));', 'args': []})
    if not result['ok']:
        raise RuntimeError(name + ': ' + result['error'])
    return result.get('value')

def check(name, condition, detail=''):
    if not condition:
        raise AssertionError(name + ': ' + str(detail))
    checks.append({'check': name, 'detail': detail})
    print('PASS', name, detail, flush=True)

def wait(predicate, limit=150, detail='condition'):
    deadline = time.monotonic() + limit
    while time.monotonic() < deadline:
        value = predicate()
        if value:
            return value
        time.sleep(.25)
    raise TimeoutError(detail)

def click(selector):
    result = js('const e=document.querySelector(' + json.dumps(selector) + ');if(!e)return false;e.click();return true;')
    check('Control present: ' + selector, result)
    time.sleep(.2)

def text_input(selector, value):
    check('Input present: ' + selector, js('const e=document.querySelector(' + json.dumps(selector) + ');if(!e)return false;Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,"value").set.call(e,' + json.dumps(value) + ');e.dispatchEvent(new Event("input",{bubbles:true}));return true;'))

def capture(name):
    time.sleep(.8)
    (output / (name + '.png')).write_bytes(base64.b64decode(request(root + '/screenshot')))
    check('Screenshot: ' + name, True)

def layout(name):
    metrics = js('const p=document.querySelector(".player-bar").getBoundingClientRect();return {width:innerWidth,height:innerHeight,scrollWidth:document.documentElement.scrollWidth,playerBottom:p.bottom,playerHeight:p.height,clipped:[...document.querySelectorAll(".player-bar button,.player-bar input")].some(e=>{const r=e.getBoundingClientRect();return r.left<0 || r.right>innerWidth || r.top<0 || r.bottom>innerHeight;})};')
    check('Layout fits: ' + name, metrics['scrollWidth'] <= metrics['width'] + 1 and not metrics['clipped'] and metrics['playerBottom'] <= metrics['height'] + 1, metrics)

def start():
    global root
    wait(lambda: request('/status'), 30, 'driver startup')
    value = request('/session', {'capabilities': {'alwaysMatch': {'tauri:options': {'application': os.path.abspath(args.application)}}}})
    root = '/session/' + value['sessionId']
    request(root + '/timeouts', {'script': 180000, 'pageLoad': 60000, 'implicit': 0})
    wait(lambda: js('return !!window.__TAURI_INTERNALS__ && !!document.querySelector(".page")'), 90, 'native app startup')
    request(root + '/window/rect', {'width': 1366, 'height': 768, 'x': 0, 'y': 0})

try:
    start()
    check('Native audio initialized', rpc('bootstrap')['audio_error'] is None)
    wait(lambda: js('return document.querySelectorAll(".track-row").length > 0'), detail='home live tracks')
    capture('01-home'); layout('1366x768 Home')
    click('[data-nav="search"]')
    text_input('input[aria-label="Search music"]', 'Scott Buckley')
    wait(lambda: js('return document.querySelectorAll(".track-row").length > 0'), detail='real SoundCloud search')
    capture('02-search')
    check('Virtualized search rows', js('return document.querySelectorAll(".track-row").length < 60'))
    click('.track-row .artist-name')
    wait(lambda: js('return document.querySelector(".entity-header h1")?.textContent === "Scott Buckley" && document.querySelectorAll(".track-row").length > 0'), detail='artist page')
    capture('03-artist')
    click('button[title="Back"]')
    wait(lambda: js('return document.querySelector("input[aria-label=\"Search music\"]")?.value === "Scott Buckley" && document.querySelectorAll(".track-row").length > 0'))
    js('document.querySelector(".track-row").dispatchEvent(new MouseEvent("contextmenu",{bubbles:true,clientX:520,clientY:230}));')
    wait(lambda: js('return !!document.querySelector(".context-menu")'))
    js('[...document.querySelectorAll(".context-menu button")].find(e=>e.textContent.trim()==="Open track").click();')
    wait(lambda: js('return !!document.querySelector(".entity-header h1") && !!document.querySelector(".entity-header .primary")'))
    capture('04-track')
    click('.entity-header .primary')
    wait(lambda: rpc('bootstrap')['player']['status'] == 'playing' and rpc('bootstrap')['player']['position_ms'] > 1500, 180, 'native playback')
    capture('05-playback')
    click('button[title="Queue · Ctrl Q"]')
    wait(lambda: js('return document.querySelectorAll(".queue-item").length > 0'))
    capture('06-queue'); layout('Queue panel')
    click('button[title="Close queue"]')
    rpc('player_control', {'action':'pause','value':None})
    wait(lambda: rpc('bootstrap')['player']['status'] == 'paused')
    rpc('player_control', {'action':'seek','value':45000})
    rpc('player_control', {'action':'volume','value':.3})
    rpc('player_control', {'action':'shuffle','value':True})
    rpc('player_control', {'action':'repeat','value':'queue'})
    player = wait(lambda: (lambda p:p if p['position_ms']>=44000 and abs(p['volume']-.3)<.001 and p['shuffle'] and p['repeat']=='queue' else None)(rpc('bootstrap')['player']))
    check('Native seek, volume, shuffle and repeat', True, {k:player[k] for k in ['position_ms','volume','shuffle','repeat']})
    click('[data-nav="library"]'); capture('07-library')
    click('[data-nav="liked"]'); click('[data-action="import"]')
    text_input('input[aria-label="SoundCloud profile"]', args.profile)
    capture('08-import-dialog')
    js('document.querySelector(".dialog form").dispatchEvent(new Event("submit",{bubbles:true,cancelable:true}));')
    wait(lambda: js('return !!document.querySelector(".import-progress")'), 30, 'import progress')
    wait(lambda: js('return !!document.querySelector(".import-progress.complete")'), 180, 'complete real public import')
    capture('09-import-complete')
    sources = rpc('library_state')['import_sources']
    source = next(s for s in sources if s['url'].rstrip('/') == 'https://soundcloud.com/' + args.profile)
    check('Actual public likes persisted', source['track_count'] > 0, {'profile':source['name'],'tracks':source['track_count'],'progress':source['progress']})
    click('button[aria-label="Close dialog"]')
    wait(lambda: js('return document.querySelectorAll(".track-row").length > 0'))
    capture('10-imported-likes'); layout('Imported likes')
    js('[...document.querySelectorAll(".library-toolbar button")].find(e=>e.textContent.includes("Sources")).click();')
    capture('11-import-sources')
    before=rpc('library_state')['favorites']
    refresh=rpc('start_likes_import',{'provider':'soundcloud','input':args.profile})
    done=wait(lambda:(lambda p:p if p and p['status'] not in ['resolving','importing','cooldown'] else None)(next((p for p in rpc('import_state') if p['job_id']==refresh['job_id']),None)),180,'refresh')
    check('Reimport is idempotent',done['status']=='complete' and done['saved']==0 and len(rpc('library_state')['favorites'])==len(before),done)
    # Play imported metadata through the unchanged native player.
    imported=next(t for t in rpc('library_state')['favorites'] if t['availability']=='playable')
    rpc('play_tracks',{'ids':[imported['internal_id']],'index':0})
    wait(lambda:rpc('bootstrap')['player']['status']=='playing',180,'imported track playback')
    check('Imported track plays natively',True,imported['title'])
    capture('12-imported-playback')
    click('[data-nav="settings"]'); capture('13-settings'); layout('Settings')
    # Settings changes must preserve the current volume and playback engine.
    settings=rpc('bootstrap')['settings'];settings['cache_limit_mb']=64;rpc('update_settings',{'settings':settings})
    check('Settings preserve volume',abs(rpc('bootstrap')['player']['volume']-.3)<.001)
    request(root + '/window/rect', {'width': 900, 'height': 620})
    capture('14-small-laptop'); layout('900x620 Settings')
    click('[data-nav="liked"]'); layout('900x620 Liked tracks');capture('15-small-likes')
    request(root + '/window/rect', {'width': 1366, 'height': 768})
    request(root, method='DELETE');root=None
    start()
    saved=rpc('library_state')
    check('Real process restart retains import',any(s['id']==source['id'] and s['track_count']==source['track_count'] for s in saved['import_sources']))
    check('Real process restart retains settings',rpc('bootstrap')['settings']['cache_limit_mb']==64)
    (output/'checks.json').write_text(json.dumps({'application':os.path.abspath(args.application),'checks':checks},indent=2),encoding='utf-8')
finally:
    if root:
        try: request(root, method='DELETE')
        except Exception: pass
