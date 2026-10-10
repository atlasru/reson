"""Real desktop E2E check; start tauri-driver with a fresh profile first.

Requires Python 3. Pass --capture-device on Linux for paced output verification.
Use a headless PulseAudio sink or real audio device; normal playback remains native.
"""
import argparse
import json, urllib.request, time, os, subprocess, base64, array, math, pathlib, uuid
parser = argparse.ArgumentParser()
parser.add_argument('--application', required=True)
parser.add_argument('--driver-url', default='http://127.0.0.1:4444')
parser.add_argument('--output', default='.smoke/desktop')
parser.add_argument('--capture-device')
options = parser.parse_args()
output_dir = pathlib.Path(options.output)
output_dir.mkdir(parents=True, exist_ok=True)
http = urllib.request.build_opener(urllib.request.ProxyHandler({}))
base = options.driver_url.rstrip("/")

def req(path='', data=None, method=None):
    body = None if data is None else json.dumps(data).encode()
    try:
        with http.open(urllib.request.Request(base + path, data=body, method=method, headers={'Content-Type': 'application/json'}), timeout=240) as response:
            result = json.load(response)
    except urllib.error.HTTPError as error:
        raise RuntimeError(error.read().decode())
    if isinstance(result.get('value'), dict) and 'error' in result['value'] and 'ok' not in result['value']:
        raise RuntimeError(result)
    return result.get('value')
for _ in range(30):
    try:
        req('/status')
        break
    except Exception:
        time.sleep(1)
root = None
checks = []

def new_session():
    global root
    value = req('/session', {'capabilities': {'alwaysMatch': {'tauri:options': {'application': os.path.abspath(options.application)}}}})
    root = '/session/' + value['sessionId']
    req(root + '/timeouts', {'script': 180000, 'pageLoad': 60000, 'implicit': 0})
    for _ in range(60):
        if js('return !!window.__TAURI_INTERNALS__ && !!document.querySelector(".page")'):
            return
        time.sleep(1)
    raise RuntimeError('Native window did not initialize')

def js(code):
    return req(root + '/execute/sync', {'script': code, 'args': []})

def invoke(name, args=None):
    return req(root + '/execute/async', {'script': 'const done=arguments[arguments.length-1]; window.__TAURI_INTERNALS__.invoke(' + json.dumps(name) + ',' + json.dumps(args or {}) + ').then(value=>done({ok:true,value}),error=>done({ok:false,error:String(error)}));', 'args': []})

def rpc(name, args=None):
    value = invoke(name, args)
    if not value['ok']:
        raise RuntimeError(name + ': ' + value['error'])
    return value.get('value')

def snapshot():
    return rpc('bootstrap')

def check(name, condition, detail=''):
    if not condition:
        raise AssertionError(name + ': ' + detail)
    checks.append(name)
    print('PASS', name, detail, flush=True)

def wait_player(predicate, limit=180):
    start = time.monotonic()
    while time.monotonic() - start < limit:
        p = snapshot()['player']
        if predicate(p):
            return p
        if p['status'] == 'error':
            raise RuntimeError(p['error'])
        time.sleep(0.5)
    raise TimeoutError('Player condition: ' + str(p))

def click(selector):
    if not js('const e=document.querySelector(' + json.dumps(selector) + ');if(!e)return false;e.click();return true;'): raise AssertionError('Missing UI control: '+selector)

def screenshot(name):
    (output_dir / (name + '.png')).write_bytes(base64.b64decode(req(root + '/screenshot')))
try:
    new_session()
    click('nav button:nth-child(2)')
    time.sleep(0.5)
    js('const e=document.querySelector("input[aria-label=\'Search music\']");Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,"value").set.call(e,"Scott Buckley");e.dispatchEvent(new Event("input",{bubbles:true}));')
    start = time.monotonic()
    while time.monotonic() - start < 180:
        rows = js('return document.querySelectorAll(".track-row").length')
        if rows:
            break
        time.sleep(1)
    check('Real SoundCloud results rendered', rows > 0, str(rows) + ' visible rows')
    click('.track-row .artist-name')
    start=time.monotonic()
    while time.monotonic()-start<120:
        if js('return !!document.querySelector(".entity-header h1") && document.querySelectorAll(".track-row").length>0'):break
        time.sleep(1)
    check('Real artist page with tracks',js('return document.querySelector(".entity-header h1")?.textContent === "Scott Buckley" && document.querySelectorAll(".track-row").length>0'))
    click('button[title="Back"]');time.sleep(.5)
    check('Back restores search query and results',js('return document.querySelector("input[aria-label=\'Search music\']")?.value === "Scott Buckley" && document.querySelectorAll(".track-row").length>0'))
    click('.tabs button:nth-child(3)');time.sleep(.5)
    click('.entity-card')
    start=time.monotonic()
    while time.monotonic()-start<180:
        if js('return !!document.querySelector(".entity-header h1") && document.querySelectorAll(".track-row").length>0'):break
        time.sleep(1)
    check('Real hydrated SoundCloud playlist page',js('return !!document.querySelector(".entity-header h1") && document.querySelectorAll(".track-row").length>0'))
    click('button[title="Back"]');time.sleep(.5)
    click('.tabs button:nth-child(1)');time.sleep(.3)
    bad=invoke('resolve_url',{'provider':'soundcloud','url':'https://soundcloud.com/reson-unavailable-'+str(uuid.uuid4())+'/track'})
    check('Real unavailable SoundCloud content stays recoverable',not bad['ok'] and 'unavailable' in bad['error'].lower())
    click('.track-row .row-number')
    p = wait_player(lambda p: p['position_ms'] > 3000 and p['status'] == 'playing')
    check('Native paced audio playback', p['current'] is not None, p['current']['title'] + ' at ' + str(p['position_ms']) + 'ms')
    rms = None
    samples = []
    if options.capture_device:
        recording = output_dir / 'sink.f32'
        with recording.open('wb') as output:
            capture = subprocess.Popen(['parec', '--device='+options.capture_device, '--format=float32le', '--rate=48000', '--channels=2'], stdout=output, stderr=subprocess.DEVNULL)
            time.sleep(6)
            capture.terminate()
            capture.wait(timeout=5)
        samples = array.array('f')
        samples.frombytes(recording.read_bytes())
        recording.unlink()
        rms = math.sqrt(sum((float(x) * x for x in samples)) / max(1, len(samples)))
        check('Real PCM reaches the output device', rms > 0.0001, f'{len(samples)} samples, RMS {rms:.6f}, peak {max(map(abs, samples), default=0):.6f}')
    screenshot('reson-search')
    click('.play-button')
    p = wait_player(lambda p: p['status'] == 'paused')
    position = p['position_ms']
    time.sleep(1)
    check('Pause stops audio position', abs(snapshot()['player']['position_ms'] - position) < 1000, str(position) + 'ms')
    click('.play-button')
    wait_player(lambda p: p['status'] == 'playing' and p['position_ms'] > position)
    check('Resume advances playback', True)
    js('const e=document.querySelector("input[aria-label=\'Playback position\']");Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,"value").set.call(e,"45000");e.dispatchEvent(new Event("input",{bubbles:true}));e.dispatchEvent(new Event("change",{bubbles:true}));e.dispatchEvent(new PointerEvent("pointerup",{bubbles:true}));')
    p = wait_player(lambda p: 44000 <= p['position_ms'] < 55000)
    check('Seek in a real HLS track', True, str(p['position_ms']) + 'ms')
    js('const e=document.querySelector("input[aria-label=\'Volume\']");Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,"value").set.call(e,"0.3");e.dispatchEvent(new Event("input",{bubbles:true}));e.dispatchEvent(new Event("change",{bubbles:true}));')
    p = wait_player(lambda p: abs(p['volume'] - 0.3) < 0.001, 5)
    check('Volume changes in Rust core', True, str(p['volume']))
    click('.sidebar-bottom button')
    time.sleep(.5)
    js('const e=document.querySelector("select[aria-label=\'Cache limit\']");e.value="64";e.dispatchEvent(new Event("change",{bubbles:true}));')
    time.sleep(.5)
    check('Settings edits preserve the current playback volume',snapshot()['settings']['cache_limit_mb']==64 and abs(snapshot()['player']['volume']-.3)<.001)
    click('button[title="Back"]')
    time.sleep(.5)
    click('button[title="Queue · Ctrl Q"]')
    time.sleep(0.5)
    queue = snapshot()['queue']
    check('Queue has multiple real tracks', len(queue['entries']) >= 3, str(len(queue['entries'])))
    last = queue['entries'][-1]['entry_id']
    rpc('queue_reorder', {'entryId': last, 'to': 1})
    check('Queue reorder', snapshot()['queue']['entries'][1]['entry_id'] == last)
    rpc('queue_remove', {'id': last})
    check('Queue remove', all((e['entry_id'] != last for e in snapshot()['queue']['entries'])))
    click('button[title="Shuffle"]')
    check('Shuffle changes core order', snapshot()['queue']['shuffle'])
    click('button[title="Repeat: off"]')
    check('Repeat queue', snapshot()['queue']['repeat'] == 'queue')
    click('button[title="Repeat: queue"]')
    check('Repeat track', snapshot()['queue']['repeat'] == 'track')
    click('button[title="Repeat: track"]')
    check('Repeat off', snapshot()['queue']['repeat'] == 'off')
    click('button[title="Shuffle"]')
    first = snapshot()['player']['entry_id']
    click('button[title="Next · Alt →"]')
    p = wait_player(lambda p: p['entry_id'] != first and p['status'] == 'playing')
    second = p['entry_id']
    check('Next plays a real next track', True, p['current']['title'])
    rpc('player_control', {'action': 'seek', 'value': 0})
    click('button[title="Previous · Alt ←"]')
    p = wait_player(lambda p: p['entry_id'] == first and p['status'] == 'playing')
    check('Previous plays the earlier track', True, p['current']['title'])
    rpc('set_favorite', {'id': p['current']['internal_id'], 'enabled': True})
    created = rpc('create_playlist', {'title': 'Validation playlist'})
    rpc('add_to_playlist', {'id': created, 'ids': [p['current']['internal_id']]})
    rpc('player_control', {'action': 'seek', 'value': 30000})
    rpc('player_control', {'action': 'pause', 'value': None})
    time.sleep(1)
    before = snapshot()
    screenshot('reson-queue')
    req(root, method='DELETE')
    root = None
    time.sleep(2)
    new_session()
    after = snapshot()
    check('Restart restores queue and current entry', after['queue']['current'] == before['queue']['current'] and len(after['queue']['entries']) == len(before['queue']['entries']))
    check('Restart restores position without autoplay', after['player']['status'] == 'paused' and abs(after['player']['position_ms'] - before['player']['position_ms']) < 2000, str(after['player']['position_ms']))
    check('Favorites, local playlist and volume persist', len(after['library']['favorites']) == 1 and len(after['library']['playlists']) == 1 and (abs(after['settings']['volume'] - 0.3) < 0.001))
    check('Random installation identity persists', after['installation']['installation_id'] == before['installation']['installation_id'])
    rpc('player_control', {'action': 'resume', 'value': None})
    wait_player(lambda p: p['status'] == 'playing' and p['position_ms'] >= 30000)
    check('Restart refreshes stream and resumes', True)
    js('document.querySelector(".play-button").focus();window.dispatchEvent(new KeyboardEvent("keydown",{code:"Space",key:" ",bubbles:true}));')
    wait_player(lambda p: p['status'] == 'paused')
    check('Keyboard Space controls playback', True)
    (output_dir / 'validation.json').write_text(json.dumps({'checks': checks, 'output_rms': rms, 'output_samples': len(samples)}, indent=2))
    print('COMPLETE', len(checks), 'checks', flush=True)
finally:
    if root:
        try:
            rpc('player_control', {'action': 'pause', 'value': None})
            req(root, method='DELETE')
        except Exception:
            pass
