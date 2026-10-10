"""Validate the release Tauri desktop, using real IPC, SoundCloud and libmpv.

Run tauri-driver first. No browser server, injected tracks or mock backend.
Screenshots are captured from the native application's WebView.
"""
import argparse
import array
import base64
import json
import math
import os
import pathlib
import subprocess
import time
import urllib.error
import urllib.request

parser = argparse.ArgumentParser()
parser.add_argument('--application', required=True)
parser.add_argument('--driver-url', default='http://127.0.0.1:4444')
parser.add_argument('--native-webkit-driver', action='store_true', help='Connect directly to WebKitWebDriver on Linux')
parser.add_argument('--output', default='.smoke/redesign')
parser.add_argument('--profile', default='scottbuckley')
parser.add_argument('--capture-device', help='Optional PulseAudio output monitor for real PCM verification')
parser.add_argument('--audio-device', help='Explicit native output for the test profile; use null on headless Windows without a sound device')
args = parser.parse_args()
output = pathlib.Path(args.output)
output.mkdir(parents=True, exist_ok=True)
http = urllib.request.build_opener(urllib.request.ProxyHandler({}))
base = args.driver_url.rstrip('/')
root = None
app_process = None
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

def key(name, code=None, ctrl=False):
    js('window.dispatchEvent(new KeyboardEvent("keydown",' + json.dumps({'key':name,'code':code or name,'ctrlKey':ctrl,'bubbles':True}) + '));')

def menu_action(index, label):
    js('document.querySelectorAll(".track-row")[' + str(index) + '].dispatchEvent(new MouseEvent("contextmenu",{bubbles:true,clientX:520,clientY:310}));')
    wait(lambda: js('return !!document.querySelector(".context-menu")'))
    check('Context action: ' + label, js('const e=[...document.querySelectorAll(".context-menu button")].find(e=>e.textContent.trim()===' + json.dumps(label) + ');if(!e)return false;e.click();return true;'))

def output_pcm():
    if not args.capture_device:
        return
    recording = output / 'sink.f32'
    try:
        with recording.open('wb') as stream:
            capture = subprocess.Popen(['parec','--device='+args.capture_device,'--format=float32le','--rate=48000','--channels=2'],stdout=stream,stderr=subprocess.DEVNULL)
            try:
                time.sleep(6)
            finally:
                capture.terminate()
                capture.wait(timeout=5)
        samples = array.array('f')
        samples.frombytes(recording.read_bytes())
        rms = math.sqrt(sum(float(x)*x for x in samples)/max(1,len(samples)))
        check('Native PCM reaches output device',rms>.0001,{'samples':len(samples),'rms':rms,'peak':max(map(abs,samples),default=0)})
    finally:
        recording.unlink(missing_ok=True)

def native_playing(require_position=False):
    player = rpc('bootstrap')['player']
    if player['status'] == 'error':
        raise RuntimeError('Native playback failed: ' + str(player['error']) + '; devices=' + json.dumps(rpc('audio_devices')))
    return player['status'] == 'playing' and (not require_position or player['position_ms'] > 1500)

def text_input(selector, value):
    check('Input present: ' + selector, js('const e=document.querySelector(' + json.dumps(selector) + ');if(!e)return false;Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,"value").set.call(e,' + json.dumps(value) + ');e.dispatchEvent(new Event("input",{bubbles:true}));return true;'))

def capture(name):
    # Give visible artwork time to finish; missing artwork remains a real fallback.
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        ready = js('const images=[...document.querySelectorAll(".artwork img")];return images.length>0 && images.every(i=>i.complete);')
        if ready:
            break
        time.sleep(.25)
    time.sleep(.4)
    (output / (name + '.png')).write_bytes(base64.b64decode(request(root + '/screenshot')))
    check('Screenshot: ' + name, True)

def layout(name):
    metrics = js('const p=document.querySelector(".player-bar").getBoundingClientRect();return {width:innerWidth,height:innerHeight,scrollWidth:document.documentElement.scrollWidth,playerBottom:p.bottom,playerHeight:p.height,clipped:[...document.querySelectorAll(".player-bar button,.player-bar input")].some(e=>{const r=e.getBoundingClientRect();return r.left<0 || r.right>innerWidth || r.top<0 || r.bottom>innerHeight;})};')
    check('Layout fits: ' + name, metrics['scrollWidth'] <= metrics['width'] + 1 and not metrics['clipped'] and metrics['playerBottom'] <= metrics['height'] + 1, metrics)

def native_window():
    import ctypes
    from ctypes import wintypes
    handles = []
    user = ctypes.windll.user32
    user.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
    user.IsWindowVisible.argtypes = [wintypes.HWND]
    callback_type = ctypes.WINFUNCTYPE(wintypes.BOOL,wintypes.HWND,wintypes.LPARAM)
    @callback_type
    def callback(handle, _):
        pid = wintypes.DWORD()
        user.GetWindowThreadProcessId(handle,ctypes.byref(pid))
        if pid.value == app_process.pid and user.IsWindowVisible(handle):
            handles.append(handle)
        return True
    user.EnumWindows(callback,0)
    return handles[0] if handles else None

def resize(width, height):
    if os.name != 'nt':
        request(root + '/window/rect', {'width':width,'height':height,'x':0,'y':0})
        return
    import ctypes
    from ctypes import wintypes
    user = ctypes.windll.user32
    handle = wait(native_window,30,'native window')
    client, outer = wintypes.RECT(), wintypes.RECT()
    user.GetClientRect.argtypes = [wintypes.HWND,ctypes.POINTER(wintypes.RECT)]
    user.GetWindowRect.argtypes = [wintypes.HWND,ctypes.POINTER(wintypes.RECT)]
    user.SetWindowPos.argtypes = [wintypes.HWND,wintypes.HWND,ctypes.c_int,ctypes.c_int,ctypes.c_int,ctypes.c_int,ctypes.c_uint]
    user.GetClientRect(handle,ctypes.byref(client));user.GetWindowRect(handle,ctypes.byref(outer))
    user.SetWindowPos(handle,None,0,0,width+(outer.right-outer.left)-(client.right-client.left),height+(outer.bottom-outer.top)-(client.bottom-client.top),0x0040)

def end():
    global root, app_process
    if root:
        request(root,method='DELETE')
        root = None
    if app_process:
        import ctypes
        from ctypes import wintypes
        handle = native_window()
        if handle:
            ctypes.windll.user32.PostMessageW.argtypes = [wintypes.HWND,ctypes.c_uint,wintypes.WPARAM,wintypes.LPARAM]
            ctypes.windll.user32.PostMessageW(handle,0x0010,0,0)
        app_process.wait(timeout=15)
        check('Native process closes cleanly',app_process.returncode==0)
        app_process = None

def start():
    global root, app_process
    def ready():
        try:
            return request('/status')
        except (urllib.error.URLError, ConnectionError):
            return False
    wait(ready, 30, 'driver startup')
    capabilities = {'tauri:options': {'application': os.path.abspath(args.application)}}
    if args.native_webkit_driver:
        if os.name == 'nt':
            raise ValueError('Native WebKitWebDriver is Linux-only')
        capabilities = {'webkitgtk:browserOptions':{'binary':os.path.abspath(args.application),'args':[]}}
    if os.name == 'nt':
        # Attach to the actual release application. The Edge launch method
        # assumes its own profile path, whereas Tauri owns its data directory.
        env = dict(os.environ)
        env['WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS'] = '--remote-debugging-port=9222 --remote-debugging-address=127.0.0.1'
        env['WEBVIEW2_USER_DATA_FOLDER'] = str((output.parent/'webview-profile').resolve())
        with (output/'desktop-app.txt').open('ab') as log:
            app_process = subprocess.Popen([os.path.abspath(args.application)],env=env,stdout=log,stderr=log)
        def webview_ready():
            if app_process.poll() is not None:
                raise RuntimeError('Release application exited before WebView2 connection')
            try:
                with http.open('http://127.0.0.1:9222/json/version',timeout=2) as response:
                    return json.load(response).get('webSocketDebuggerUrl')
            except (urllib.error.URLError, ConnectionError):
                return False
        wait(webview_ready,60,'release WebView2 remote connection')
        # Pass through the native attach capabilities; tauri:options would
        # replace ms:edgeOptions with its application-launch configuration.
        capabilities.pop('tauri:options')
        capabilities['browserName'] = 'webview2'
        capabilities['ms:edgeChromium'] = True
        capabilities['ms:edgeOptions'] = {'debuggerAddress':'127.0.0.1:9222'}
    value = request('/session', {'capabilities': {'alwaysMatch': capabilities}})
    root = '/session/' + value['sessionId']
    request(root + '/timeouts', {'script': 180000, 'pageLoad': 60000, 'implicit': 0})
    wait(lambda: js('return !!window.__TAURI_INTERNALS__ && !!document.querySelector(".page")'), 90, 'native app startup')
    if args.audio_device:
        devices = rpc('audio_devices')
        settings = rpc('bootstrap')['settings']
        settings['audio_device'] = args.audio_device
        rpc('update_settings', {'settings':settings})
        check('Explicit native test output selected',rpc('bootstrap')['settings']['audio_device'] == args.audio_device,{'selected':args.audio_device,'available':devices})
    resize(1366,768)

try:
    start()
    check('Native audio initialized', rpc('bootstrap')['audio_error'] is None)
    wait(lambda: js('return document.querySelectorAll(".track-row").length > 0'), detail='home live tracks')
    capture('01-home'); layout('1366x768 Home')
    key('k','KeyK',ctrl=True)
    wait(lambda: js('return document.activeElement?.getAttribute("aria-label") === "Search music"'))
    check('Ctrl K opens and focuses search', True)
    text_input('input[aria-label="Search music"]', 'Scott Buckley')
    wait(lambda: js('return document.querySelectorAll(".track-row").length > 0'), detail='real SoundCloud search')
    capture('02-search')
    check('Virtualized search rows', js('return document.querySelectorAll(".track-row").length < 60'))
    click('.track-row .artist-name')
    wait(lambda: js('return document.querySelector(".entity-header h1")?.textContent === "Scott Buckley" && document.querySelectorAll(".track-row").length > 0'), detail='artist page')
    capture('03-artist')
    click('button[title="Back"]')
    wait(lambda: js("""return document.querySelector('input[aria-label="Search music"]')?.value === "Scott Buckley" && document.querySelectorAll('.track-row').length > 0"""))
    js('document.querySelector(".track-row").dispatchEvent(new MouseEvent("contextmenu",{bubbles:true,clientX:520,clientY:230}));')
    wait(lambda: js('return !!document.querySelector(".context-menu")'))
    js('[...document.querySelectorAll(".context-menu button")].find(e=>e.textContent.trim()==="Open track").click();')
    wait(lambda: js('return !!document.querySelector(".entity-header h1") && !!document.querySelector(".entity-header .primary")'))
    wait(lambda: js('return document.querySelectorAll(".track-row").length > 1'), detail='real related tracks')
    capture('04-track')
    click('.entity-header .primary')
    wait(lambda: native_playing(True), 180, 'native playback')
    output_pcm()
    capture('05-playback')
    key(' ','Space')
    wait(lambda: rpc('bootstrap')['player']['status'] == 'paused')
    key(' ','Space')
    wait(lambda: rpc('bootstrap')['player']['status'] == 'playing')
    check('Space pauses and resumes native playback', True)
    menu_action(0,'Play next')
    menu_action(1,'Add to queue')
    wait(lambda: len(rpc('bootstrap')['queue']['entries']) == 3)
    check('Context menus update the native queue',True)
    click('button[title="Queue · Ctrl Q"]')
    wait(lambda: js('return document.querySelectorAll(".queue-item").length > 0'))
    capture('06-queue'); layout('Queue panel')
    last = rpc('bootstrap')['queue']['entries'][-1]['entry_id']
    click('.queue-item:last-child button[title="Move up"]')
    wait(lambda: rpc('bootstrap')['queue']['entries'][1]['entry_id'] == last)
    click('.queue-item:last-child button[title="Remove queue entry"]')
    wait(lambda: len(rpc('bootstrap')['queue']['entries']) == 2)
    check('Native queue reorder and removal through UI',True)
    key('Escape')
    wait(lambda: js('return !document.querySelector(".queue-panel")'))
    check('Escape closes queue',True)
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
    click('.import-source button[title^="Refresh "]')
    wait(lambda: js('return !!document.querySelector(".import-progress.complete")'),180,'refresh')
    done = rpc('library_state')['import_sources'][0]['progress']
    check('Reimport is idempotent',done['status']=='complete' and done['saved']==0 and len(rpc('library_state')['favorites'])==len(before),done)
    key('Escape')
    wait(lambda: js('return !document.querySelector(".dialog")'))
    # Double-click an imported row through the real UI and native player.
    js('document.querySelector(".track-row").dispatchEvent(new MouseEvent("dblclick",{bubbles:true}));')
    wait(native_playing,180,'imported track playback')
    imported=rpc('bootstrap')['player']['current']
    check('Double-click plays a saved import',any(t['internal_id']==imported['internal_id'] for t in before))
    check('Imported track plays natively',True,imported['title'])
    capture('12-imported-playback')
    click('[data-nav="settings"]'); capture('13-settings'); layout('Settings')
    # Settings changes must preserve the current volume and playback engine.
    settings=rpc('bootstrap')['settings'];settings['cache_limit_mb']=64;rpc('update_settings',{'settings':settings})
    check('Settings preserve volume',abs(rpc('bootstrap')['player']['volume']-.3)<.001)
    resize(900,620)
    capture('14-small-laptop'); layout('900x620 Settings')
    click('[data-nav="liked"]'); layout('900x620 Liked tracks');capture('15-small-likes')
    resize(1366,768)
    end()
    start()
    saved=rpc('library_state')
    check('Real process restart retains import',any(s['id']==source['id'] and s['track_count']==source['track_count'] for s in saved['import_sources']))
    check('Real process restart retains settings',rpc('bootstrap')['settings']['cache_limit_mb']==64)
    (output/'checks.json').write_text(json.dumps({'application':os.path.abspath(args.application),'checks':checks},indent=2),encoding='utf-8')
except Exception:
    if root:
        try:
            capture('failure')
            (output/'failure-state.json').write_text(json.dumps({'text':js('return document.body.innerText'),'state':rpc('bootstrap'),'checks':checks},indent=2),encoding='utf-8')
        except Exception:
            pass
    raise
finally:
    try:
        end()
    except Exception:
        if app_process and app_process.poll() is None:
            app_process.kill()
