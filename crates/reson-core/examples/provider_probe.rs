use reson_core::{
    audio::{AudioCommand, AudioEvent, NativeAudio},
    error::Result,
    providers::{soundcloud::SoundCloudProvider, MusicProvider},
};
use tokio_util::sync::CancellationToken;
#[tokio::main]
async fn main() -> Result<()> {
    let provider = SoundCloudProvider::new()?;
    let results = provider
        .search("Scott Buckley", 0, CancellationToken::new())
        .await?;
    println!(
        "Search: {} tracks, {} artists, {} playlists",
        results.tracks.len(),
        results.artists.len(),
        results.playlists.len()
    );
    let track = results
        .tracks
        .iter()
        .find(|t| t.availability == reson_core::models::Availability::Playable)
        .ok_or(reson_core::error::Error::Unavailable)?;
    println!("Track: {} / {}", track.title, track.artist_name());
    let source = provider.resolve_stream(&track.sources[0]).await?;
    println!("Resolved {:?} source", source.kind);std::fs::write("/tmp/reson-native-source.tmp",&source.url)?;
    let output = std::env::temp_dir().join("reson-probe-audio.wav");
    let (audio, mut events) = NativeAudio::start(None, Some(output.clone()))?;
    audio.send(AudioCommand::Load {
        source,
        position_ms: 0,
        paused: false,
    })?;
    let timeout = tokio::time::sleep(std::time::Duration::from_secs(120));
    tokio::pin!(timeout);
    loop {
        tokio::select! { _=&mut timeout=>{println!("Probe timeout");break;}, Some(e)=events.recv()=>match e {AudioEvent::Loaded=>println!("Native file loaded"),AudioEvent::Position(p) if p>3000=>{println!("Native position {p}ms");break;},AudioEvent::Error(e)=>{println!("Native error: {e}");break;},_=>{}} }
    }
    audio.send(AudioCommand::Stop)?;
    audio.send(AudioCommand::Shutdown)?;
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    println!(
        "PCM output bytes {}",
        std::fs::metadata(&output).map(|m| m.len()).unwrap_or(0)
    );
    if output.exists() {
        std::fs::remove_file(output)?;
    }
    match provider.discover().await {
        Ok(t) => println!("Discovery: {} tracks", t.len()),
        Err(e) => println!("Discovery: {e}"),
    };
    Ok(())
}
