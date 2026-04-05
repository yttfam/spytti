use serde::Serialize;

#[derive(Serialize, Clone, Debug)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    pub card: u32,
    pub rates: Vec<u32>,
}

/// List available ALSA playback devices by parsing `aplay -l`.
/// Returns an empty vec on non-Linux or if aplay isn't available.
pub fn list_devices() -> Vec<AudioDevice> {
    let output = match std::process::Command::new("aplay").arg("-l").output() {
        Ok(o) => o,
        Err(_) => return vec![],
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut devices = Vec::new();

    for line in stdout.lines() {
        if !line.starts_with("card ") {
            continue;
        }

        let Some((card_num, name)) = parse_card_line(line) else {
            continue;
        };

        let rates = detect_sample_rates(card_num);

        devices.push(AudioDevice {
            id: format!("default:CARD={card_num}"),
            name,
            card: card_num,
            rates,
        });
    }

    devices
}

/// Read supported sample rates from /proc/asound/cardN/stream0.
fn detect_sample_rates(card: u32) -> Vec<u32> {
    let path = format!("/proc/asound/card{card}/stream0");
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    let mut rates = Vec::new();
    let mut in_playback = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "Playback:" {
            in_playback = true;
        } else if trimmed == "Capture:" {
            in_playback = false;
        }
        if in_playback && trimmed.starts_with("Rates:") {
            if let Some(rates_str) = trimmed.strip_prefix("Rates:") {
                for rate in rates_str.split_whitespace() {
                    if let Ok(r) = rate.parse::<u32>() {
                        if !rates.contains(&r) {
                            rates.push(r);
                        }
                    }
                }
            }
        }
    }

    rates.sort();
    rates
}

fn parse_card_line(line: &str) -> Option<(u32, String)> {
    let after_card = line.strip_prefix("card ")?;
    let colon_pos = after_card.find(':')?;
    let card_num: u32 = after_card[..colon_pos].parse().ok()?;

    let bracket_start = line.find('[')?;
    let bracket_end = line.find(']')?;
    let name = line[bracket_start + 1..bracket_end].to_string();

    Some((card_num, name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_aplay_line() {
        let line = "card 0: Device [USB2.0 Device], device 0: USB Audio [USB Audio]";
        let (card, name) = parse_card_line(line).unwrap();
        assert_eq!(card, 0);
        assert_eq!(name, "USB2.0 Device");
    }

    #[test]
    fn parse_hdmi_line() {
        let line = "card 1: vc4hdmi [vc4-hdmi], device 0: MAI PCM i2s-hifi-0 [MAI PCM i2s-hifi-0]";
        let (card, name) = parse_card_line(line).unwrap();
        assert_eq!(card, 1);
        assert_eq!(name, "vc4-hdmi");
    }

    #[test]
    fn parse_invalid_line() {
        assert!(parse_card_line("  Subdevices: 1/1").is_none());
        assert!(parse_card_line("").is_none());
    }

    #[test]
    fn parse_sample_rates_from_stream() {
        // Simulates /proc/asound/cardN/stream0 content
        let content = "Playback:\n  Interface 1\n    Rates: 44100 48000 96000\n    Bits: 16\nCapture:\n    Rates: 48000\n";
        let mut rates = Vec::new();
        let mut in_playback = false;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed == "Playback:" { in_playback = true; }
            if trimmed == "Capture:" { in_playback = false; }
            if in_playback {
                if let Some(rates_str) = trimmed.strip_prefix("Rates:") {
                    for rate in rates_str.split_whitespace() {
                        if let Ok(r) = rate.parse::<u32>() {
                            rates.push(r);
                        }
                    }
                }
            }
        }
        assert_eq!(rates, vec![44100, 48000, 96000]);
    }
}
