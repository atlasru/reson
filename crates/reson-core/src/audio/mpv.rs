use super::*;
use crate::error::Error;
use libloading::Library;
use std::{
    ffi::{c_char, c_int, c_void, CStr, CString},
    sync::mpsc as sync_mpsc,
    time::{Duration, Instant},
};

#[repr(C)]
struct MpvEvent {
    id: c_int,
    error: c_int,
    userdata: u64,
    data: *mut c_void,
}
#[repr(C)]
struct MpvProperty {
    name: *const c_char,
    format: c_int,
    data: *mut c_void,
}
#[repr(C)]
struct MpvLog {
    prefix: *const c_char,
    level: *const c_char,
    text: *const c_char,
    log_level: c_int,
}
#[repr(C)]
struct MpvEndFile {
    reason: c_int,
    error: c_int,
}
#[repr(C)]
union MpvValue {
    string: *mut c_char,
    integer: i64,
    double: f64,
    list: *mut MpvNodeList,
}
#[repr(C)]
struct MpvNode {
    value: MpvValue,
    format: c_int,
}
#[repr(C)]
struct MpvNodeList {
    count: c_int,
    values: *mut MpvNode,
    keys: *mut *mut c_char,
}
struct Api {
    _library: Library,
    create: unsafe extern "C" fn() -> *mut c_void,
    initialize: unsafe extern "C" fn(*mut c_void) -> c_int,
    option: unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> c_int,
    command: unsafe extern "C" fn(*mut c_void, *const *const c_char) -> c_int,
    property: unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> c_int,
    observe: unsafe extern "C" fn(*mut c_void, u64, *const c_char, c_int) -> c_int,
    wait: unsafe extern "C" fn(*mut c_void, f64) -> *const MpvEvent,
    wake_callback:
        unsafe extern "C" fn(*mut c_void, Option<unsafe extern "C" fn(*mut c_void)>, *mut c_void),
    destroy: unsafe extern "C" fn(*mut c_void),
    request_log: unsafe extern "C" fn(*mut c_void, *const c_char) -> c_int,
    get_property: unsafe extern "C" fn(*mut c_void, *const c_char, c_int, *mut c_void) -> c_int,
    free_node: unsafe extern "C" fn(*mut MpvNode),
    error_string: unsafe extern "C" fn(c_int) -> *const c_char,
}
impl Api {
    // All FFI pointers are resolved once from an owned library, used on one worker,
    // and never survive its destructor. Struct layouts follow mpv client.h API v2.
    unsafe fn load(path: Option<PathBuf>) -> Result<Self> {
        let candidates = if let Some(path) = path {
            vec![path]
        } else if cfg!(target_os = "windows") {
            let dir = std::env::current_exe()?
                .parent()
                .ok_or_else(|| Error::Audio("Missing executable directory".into()))?
                .to_path_buf();
            vec![dir.join("libmpv-2.dll"), dir.join("resources/libmpv-2.dll")]
        } else if cfg!(target_os = "macos") {
            vec![PathBuf::from("libmpv.2.dylib")]
        } else {
            vec![PathBuf::from("libmpv.so.2"), PathBuf::from("libmpv.so.1")]
        };
        let library = candidates
            .iter()
            .find_map(|path| Library::new(path).ok())
            .ok_or_else(|| {
                Error::Audio(
                    "Native audio library missing. Reinstall Reson (Linux: install libmpv).".into(),
                )
            })?;
        let version: unsafe extern "C" fn() -> u64 = *library
            .get(b"mpv_client_api_version\0")
            .map_err(|_| Error::Audio("Invalid native audio library".into()))?;
        if version() >> 16 != 2 {
            return Err(Error::Audio("Unsupported native audio ABI".into()));
        }
        macro_rules! symbol {
            ($name:literal) => {
                *library.get(concat!($name, "\0").as_bytes()).map_err(|_| {
                    Error::Audio(concat!("Missing native audio symbol: ", $name).into())
                })?
            };
        }
        Ok(Self {
            create: symbol!("mpv_create"),
            initialize: symbol!("mpv_initialize"),
            option: symbol!("mpv_set_option_string"),
            command: symbol!("mpv_command"),
            property: symbol!("mpv_set_property_string"),
            observe: symbol!("mpv_observe_property"),
            wait: symbol!("mpv_wait_event"),
            wake_callback: symbol!("mpv_set_wakeup_callback"),
            destroy: symbol!("mpv_terminate_destroy"),
            request_log: symbol!("mpv_request_log_messages"),
            get_property: symbol!("mpv_get_property"),
            free_node: symbol!("mpv_free_node_contents"),
            error_string: symbol!("mpv_error_string"),
            _library: library,
        })
    }
}
struct Mpv {
    api: Api,
    handle: *mut c_void,
    notifier: Box<sync_mpsc::Sender<Message>>,
}
impl Mpv {
    fn new(
        path: Option<PathBuf>,
        output: Option<PathBuf>,
        sender: sync_mpsc::Sender<Message>,
    ) -> Result<Self> {
        let api = unsafe { Api::load(path)? };
        let handle = unsafe { (api.create)() };
        if handle.is_null() {
            return Err(Error::Audio("Native audio initialization failed".into()));
        }
        let this = Self {
            api,
            handle,
            notifier: Box::new(sender),
        };
        for (key, value) in [
            ("config", "no"),
            ("load-scripts", "no"),
            ("ytdl", "no"),
            ("terminal", "no"),
            ("osc", "no"),
            ("video", "no"),
            ("audio-display", "no"),
            ("idle", "yes"),
            ("input-default-bindings", "no"),
            ("input-media-keys", "no"),
            ("network-timeout", "15"),
            ("tls-verify", "yes"),
            ("cache", "yes"),
            ("cache-secs", "20"),
            ("demuxer-max-bytes", "16MiB"),
            ("demuxer-max-back-bytes", "8MiB"),
            ("audio-fallback-to-null", "no"),
        ] {
            this.option(key, value)?;
        }
        if let Ok(proxy) = std::env::var("https_proxy").or_else(|_| std::env::var("HTTPS_PROXY")) {
            this.option("http-proxy", &proxy)?;
        }
        if let Ok(cert) = std::env::var("SSL_CERT_FILE") {
            this.option("tls-ca-file", &cert)?;
        }
        if let Some(output) = output {
            this.option("ao", "pcm")?;
            this.option("ao-pcm-file", &output.to_string_lossy())?;
            this.option("ao-pcm-waveheader", "no")?;
            this.option("audio-format", "float")?;
            this.option("audio-samplerate", "48000")?;
            this.option("audio-channels", "stereo")?;
        }
        this.check(unsafe { (this.api.initialize)(handle) })?;
        this.check(unsafe { (this.api.request_log)(handle, c"warn".as_ptr()) })?;
        // Property notifications drive synchronization. Idle waits on a channel.
        for (index, (name, format)) in [
            ("time-pos", 5),
            ("duration", 5),
            ("pause", 3),
            ("paused-for-cache", 3),
        ]
        .into_iter()
        .enumerate()
        {
            let name = CString::new(name).expect("fixed name");
            this.check(unsafe {
                (this.api.observe)(handle, index as u64 + 1, name.as_ptr(), format)
            })?;
        }
        unsafe {
            (this.api.wake_callback)(
                handle,
                Some(wakeup),
                (&*this.notifier as *const sync_mpsc::Sender<Message>)
                    .cast_mut()
                    .cast(),
            )
        };
        Ok(this)
    }
    fn check(&self, code: c_int) -> Result<()> {
        if code >= 0 {
            return Ok(());
        }
        let message = unsafe { CStr::from_ptr((self.api.error_string)(code)) }
            .to_string_lossy()
            .into_owned();
        Err(Error::Audio(message))
    }
    fn option(&self, name: &str, value: &str) -> Result<()> {
        let name = cstring(name)?;
        let value = cstring(value)?;
        self.check(unsafe { (self.api.option)(self.handle, name.as_ptr(), value.as_ptr()) })
    }
    fn property(&self, name: &str, value: &str) -> Result<()> {
        let name = cstring(name)?;
        let value = cstring(value)?;
        self.check(unsafe { (self.api.property)(self.handle, name.as_ptr(), value.as_ptr()) })
    }
    fn command(&self, args: &[&str]) -> Result<()> {
        let values = args
            .iter()
            .map(|s| cstring(s))
            .collect::<Result<Vec<_>>>()?;
        let pointers = values
            .iter()
            .map(|s| s.as_ptr())
            .chain(std::iter::once(std::ptr::null()))
            .collect::<Vec<_>>();
        self.check(unsafe { (self.api.command)(self.handle, pointers.as_ptr()) })
    }
    fn devices(&self) -> Result<Vec<AudioDevice>> {
        let mut node = MpvNode {
            value: MpvValue { integer: 0 },
            format: 0,
        };
        self.check(unsafe {
            (self.api.get_property)(
                self.handle,
                c"audio-device-list".as_ptr(),
                6,
                (&mut node as *mut MpvNode).cast(),
            )
        })?;
        let mut devices = vec![AudioDevice {
            id: "auto".into(),
            name: "System default".into(),
        }];
        unsafe {
            if node.format == 7 && !node.value.list.is_null() {
                let list = &*node.value.list;
                for i in 0..list.count.clamp(0, 256) as usize {
                    let value = &*list.values.add(i);
                    if value.format != 8 || value.value.list.is_null() {
                        continue;
                    }
                    let map = &*value.value.list;
                    let mut id = None;
                    let mut name = None;
                    for j in 0..map.count.clamp(0, 32) as usize {
                        let key = *map.keys.add(j);
                        let val = &*map.values.add(j);
                        if key.is_null() || val.format != 1 || val.value.string.is_null() {
                            continue;
                        }
                        let text = CStr::from_ptr(val.value.string)
                            .to_string_lossy()
                            .into_owned();
                        match CStr::from_ptr(key).to_bytes() {
                            b"name" => id = Some(text),
                            b"description" => name = Some(text),
                            _ => {}
                        }
                    }
                    if let Some(id) = id {
                        if id != "auto" {
                            devices.push(AudioDevice {
                                name: name.unwrap_or_else(|| id.clone()),
                                id,
                            });
                        }
                    }
                }
            }
            (self.api.free_node)(&mut node);
        }
        Ok(devices)
    }
    fn execute(&self, command: AudioCommand) -> Result<()> {
        match command {
            AudioCommand::Load {
                source,
                position_ms,
                paused,
            } => {
                let _ = (position_ms, paused);
                self.property("pause", "yes")?;
                self.command(&["loadfile", &source.url, "replace"])
            }
            AudioCommand::Pause(value) => self.property("pause", if value { "yes" } else { "no" }),
            AudioCommand::Stop => self.command(&["stop"]),
            AudioCommand::Seek(pos) => self.command(&[
                "seek",
                &format!("{:.3}", pos as f64 / 1000.0),
                "absolute+exact",
            ]),
            AudioCommand::Volume(volume) => self.property("volume", &format!("{}", volume * 100.0)),
            AudioCommand::Device(device) => self.property("audio-device", &device),
            AudioCommand::Shutdown => Ok(()),
        }
    }
}
impl Drop for Mpv {
    fn drop(&mut self) {
        unsafe {
            (self.api.wake_callback)(self.handle, None, std::ptr::null_mut());
            (self.api.destroy)(self.handle);
        }
    }
}
unsafe extern "C" fn wakeup(data: *mut c_void) {
    // Sender stays alive until callback is unset and mpv is fully terminated.
    if let Some(sender) = data.cast::<sync_mpsc::Sender<Message>>().as_ref() {
        let _ = sender.send(Message::Wake);
    }
}
fn cstring(value: &str) -> Result<CString> {
    CString::new(value).map_err(|_| Error::Invalid("Audio argument contains a null byte".into()))
}
enum Message {
    Command(AudioCommand),
    Devices(sync_mpsc::Sender<Result<Vec<AudioDevice>>>),
    Wake,
}
struct Handle(sync_mpsc::Sender<Message>);
impl AudioBackend for Handle {
    fn send(&self, command: AudioCommand) -> Result<()> {
        self.0
            .send(Message::Command(command))
            .map_err(|_| Error::Audio("Audio worker stopped".into()))
    }
    fn devices(&self) -> Result<Vec<AudioDevice>> {
        let (tx, rx) = sync_mpsc::channel();
        self.0
            .send(Message::Devices(tx))
            .map_err(|_| Error::Audio("Audio worker stopped".into()))?;
        rx.recv_timeout(Duration::from_secs(2))
            .map_err(|_| Error::Audio("Audio device enumeration timed out".into()))?
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        let _ = self.0.send(Message::Command(AudioCommand::Shutdown));
    }
}

pub fn start(
    path: Option<PathBuf>,
    output: Option<PathBuf>,
) -> Result<(Arc<dyn AudioBackend>, mpsc::UnboundedReceiver<AudioEvent>)> {
    let (sender, receiver) = sync_mpsc::channel();
    let (ready_tx, ready_rx) = sync_mpsc::sync_channel(1);
    let (events_tx, events_rx) = mpsc::unbounded_channel();
    let notifier = sender.clone();
    std::thread::Builder::new().name("reson-audio".into()).spawn(move||{
        let mpv=match Mpv::new(path,output,notifier){Ok(mpv)=>{let _=ready_tx.send(Ok(()));mpv},Err(e)=>{let _=ready_tx.send(Err(e));return;}};
        let mut last_position=Instant::now()-Duration::from_secs(1);
        let mut pending_load=None;
        while let Ok(message)=receiver.recv() {
            if let Message::Devices(ref reply)=message {let _=reply.send(mpv.devices());}
            if let Message::Command(command)=message {
                if matches!(command,AudioCommand::Shutdown){break;}
                if let AudioCommand::Load{position_ms,paused,..}=&command{pending_load=Some((*position_ms,*paused));}
                if let AudioCommand::Pause(paused)=&command{if let Some((_,pending))=&mut pending_load{*pending = *paused;}}
                if matches!(command,AudioCommand::Stop){pending_load=None;}
                if let Err(e)=mpv.execute(command){let _=events_tx.send(AudioEvent::Error(e.to_string()));}
            }
            loop {
                let event=unsafe{(mpv.api.wait)(mpv.handle,0.0)};
                if event.is_null(){break;}let event=unsafe{&*event};if event.id==0 {break;}
                let value=match event.id {
                    2 if !event.data.is_null()=> {
                        let log=unsafe{&*event.data.cast::<MpvLog>()};
                        if !log.text.is_null(){
                            let message=unsafe{CStr::from_ptr(log.text)}.to_string_lossy().to_lowercase();
                            let category=if message.contains("certificate")||message.contains("tls"){"tls"}else if message.contains("audio")||message.contains("device"){"audio_device"}else if message.contains("http")||message.contains("network"){"network"}else{"decoder"};
                            tracing::debug!(category,severity=log.log_level,"native playback diagnostic");
                        }
                        None
                    },
                    8=>{
                        if let Some((position,paused))=pending_load.take(){
                            let result=(||{if position>0{mpv.execute(AudioCommand::Seek(position))?;}mpv.execute(AudioCommand::Pause(paused))})();
                            if let Err(e)=result {let _=events_tx.send(AudioEvent::Error(e.to_string()));}
                        }
                        Some(AudioEvent::Loaded)
                    },
                    7 if !event.data.is_null()=> {let end=unsafe{&*event.data.cast::<MpvEndFile>()};match end.reason {0=>Some(AudioEvent::Ended),4=>Some(AudioEvent::Error("Unable to read the audio stream. Check the network or choose another track.".into())),_=>None}},
                    22 if !event.data.is_null()=> {
                        let p=unsafe{&*event.data.cast::<MpvProperty>()};if p.name.is_null()||p.data.is_null(){continue;}
                        let name=unsafe{CStr::from_ptr(p.name)}.to_bytes();
                        match (name,p.format) {
                            (b"time-pos",5) if last_position.elapsed()>=Duration::from_millis(450)=> {last_position=Instant::now();Some(AudioEvent::Position(seconds(unsafe{*p.data.cast::<f64>()})))},
                            (b"duration",5)=>Some(AudioEvent::Duration(seconds(unsafe{*p.data.cast::<f64>()}))),
                            (b"pause",3)=>Some(AudioEvent::Paused(unsafe{*p.data.cast::<c_int>()}!=0)),
                            (b"paused-for-cache",3)=>Some(AudioEvent::Buffering(unsafe{*p.data.cast::<c_int>()}!=0)),
                            _=>None,
                        }
                    }
                    _=>None,
                };
                if let Some(value)=value{if events_tx.send(value).is_err(){return;}}
            }
        }
    })?;
    ready_rx
        .recv()
        .map_err(|_| Error::Audio("Audio initialization interrupted".into()))??;
    Ok((Arc::new(Handle(sender)), events_rx))
}
fn seconds(seconds: f64) -> u64 {
    if seconds.is_finite() {
        (seconds.max(0.0) * 1000.0) as u64
    } else {
        0
    }
}
