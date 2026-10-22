//! Milestone 4: the panda's voice, using the Windows built-in voices
//! (WinRT `SpeechSynthesizer`): offline, free, already installed, and fast
//! enough to speak within a second of a trigger.
//!
//! The spec suggests Kokoro or Piper; both need the GPL espeak-ng phonemiser
//! and a 100+ MB model, so they're left for v1. This uses the spec's fallback
//! (the system voices) as the default.

use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

/// The panda talks a little slower than normal: sleepy, not slow.
pub const PANDA_RATE: f64 = 0.9;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceInfo {
    pub id: String,
    pub name: String,
    pub language: String,
}

/// A line that started playing.
#[derive(Debug, Clone, PartialEq)]
pub struct Spoken {
    /// From the request to the moment playback started.
    pub latency: Duration,
    /// How long the audio lasts (for the talking animation).
    pub duration: Duration,
}

struct Job {
    text: String,
    voice: String,
    rate: f64,
    play: bool,
    reply: Sender<Result<Spoken, String>>,
}

/// Owns the speech thread. Cheap to share by reference; jobs are queued.
pub struct Speaker {
    tx: Sender<Job>,
}

impl Speaker {
    pub fn start() -> Speaker {
        let (tx, rx) = mpsc::channel::<Job>();
        std::thread::Builder::new()
            .name("nudgy-voice".into())
            .spawn(move || platform::run(rx))
            .expect("spawn voice thread");
        Speaker { tx }
    }

    /// Speaks `text`; the receiver yields once playback has started.
    /// `voice` is a voice id from `voices()`, or "default".
    pub fn speak(&self, text: &str, voice: &str) -> Receiver<Result<Spoken, String>> {
        self.job(text, voice, true)
    }

    /// Synthesises without playing: for latency checks.
    pub fn synthesize(&self, text: &str, voice: &str) -> Receiver<Result<Spoken, String>> {
        self.job(text, voice, false)
    }

    fn job(&self, text: &str, voice: &str, play: bool) -> Receiver<Result<Spoken, String>> {
        let (reply, rx) = mpsc::channel();
        let job = Job { text: text.to_string(), voice: voice.to_string(), rate: PANDA_RATE, play, reply };
        if let Err(e) = self.tx.send(job) {
            let _ = e.0.reply.send(Err("voice thread stopped".into()));
        }
        rx
    }
}

/// Installed voices, English first.
pub fn voices() -> Vec<VoiceInfo> {
    let mut list = platform::voices();
    list.sort_by_key(|v| (!v.language.starts_with("en"), v.name.clone()));
    list
}

#[cfg(windows)]
mod platform {
    use std::sync::mpsc::Receiver;
    use std::time::{Duration, Instant};

    use windows::Media::Core::MediaSource;
    use windows::Media::Playback::MediaPlayer;
    use windows::Media::SpeechSynthesis::{SpeechSynthesisStream, SpeechSynthesizer, VoiceInformation};
    use windows::Storage::Streams::DataReader;
    use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
    use windows::core::HSTRING;

    use super::{Job, Spoken, VoiceInfo};

    pub fn voices() -> Vec<VoiceInfo> {
        let Ok(all) = SpeechSynthesizer::AllVoices() else { return vec![] };
        all.into_iter()
            .filter_map(|v| {
                Some(VoiceInfo {
                    id: v.Id().ok()?.to_string(),
                    name: v.DisplayName().ok()?.to_string(),
                    language: v.Language().ok()?.to_string(),
                })
            })
            .collect()
    }

    fn find_voice(id: &str) -> Option<VoiceInformation> {
        if id.is_empty() || id == "default" {
            return None;
        }
        SpeechSynthesizer::AllVoices().ok()?.into_iter().find(|v| v.Id().is_ok_and(|vid| vid.to_string() == id))
    }

    pub fn run(jobs: Receiver<Job>) {
        // WinRT media objects need COM on this thread.
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
        let mut player: Option<MediaPlayer> = None;
        for job in jobs {
            let started = Instant::now();
            let result = speak(&job, &mut player).map(|duration| Spoken { latency: started.elapsed(), duration });
            if let Err(e) = &result {
                log::warn!("speech failed: {e}");
            }
            let _ = job.reply.send(result);
        }
    }

    fn speak(job: &Job, player: &mut Option<MediaPlayer>) -> Result<Duration, String> {
        let err = |e: windows::core::Error| e.message().to_string();
        let synth = SpeechSynthesizer::new().map_err(err)?;
        if let Some(voice) = find_voice(&job.voice) {
            synth.SetVoice(&voice).map_err(err)?;
        }
        synth.Options().and_then(|o| o.SetSpeakingRate(job.rate)).map_err(err)?;
        let stream: SpeechSynthesisStream =
            synth.SynthesizeTextToStreamAsync(&HSTRING::from(job.text.as_str())).map_err(err)?.join().map_err(err)?;
        let duration = wav_duration(&stream).unwrap_or_default();
        if job.play {
            if player.is_none() {
                *player = Some(MediaPlayer::new().map_err(err)?);
            }
            let player = player.as_ref().unwrap();
            let source = MediaSource::CreateFromStream(&stream, &stream.ContentType().map_err(err)?).map_err(err)?;
            // A new line replaces one still playing: the newest moment wins.
            player.SetSource(&source).map_err(err)?;
            player.Play().map_err(err)?;
        }
        Ok(duration)
    }

    /// Length of the WAV the synthesiser produced, from its header.
    fn wav_duration(stream: &SpeechSynthesisStream) -> Option<Duration> {
        let size = stream.Size().ok()?;
        let reader = DataReader::CreateDataReader(&stream.GetInputStreamAt(0).ok()?).ok()?;
        reader.LoadAsync(44).ok()?.join().ok()?;
        let mut header = [0u8; 44];
        reader.ReadBytes(&mut header).ok()?;
        let byte_rate = u32::from_le_bytes(header[28..32].try_into().ok()?) as u64;
        (byte_rate > 0).then(|| Duration::from_millis(size.saturating_sub(44) * 1000 / byte_rate))
    }
}

#[cfg(not(windows))]
mod platform {
    use std::sync::mpsc::Receiver;

    use super::{Job, VoiceInfo};

    pub fn voices() -> Vec<VoiceInfo> {
        vec![]
    }

    pub fn run(jobs: Receiver<Job>) {
        for job in jobs {
            let _ = job.reply.send(Err("speech is only available on Windows".into()));
        }
    }
}
