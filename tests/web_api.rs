//! Integration tests for spytti's web API contract and static assets.

#[test]
fn example_config_is_valid_toml() {
    let toml_str = include_str!("../spytti.toml");
    let parsed: toml::Value = toml::from_str(toml_str).expect("spytti.toml should be valid TOML");
    assert_eq!(parsed.get("name").unwrap().as_str().unwrap(), "Spotify Salon");
    assert_eq!(parsed.get("bitrate").unwrap().as_integer().unwrap(), 320);
    assert_eq!(parsed.get("port").unwrap().as_integer().unwrap(), 8080);
    assert_eq!(parsed.get("initial_volume").unwrap().as_integer().unwrap(), 30);
}

#[test]
fn systemd_service_has_required_sections() {
    let service = include_str!("../spytti.service");
    assert!(service.contains("[Unit]"));
    assert!(service.contains("[Service]"));
    assert!(service.contains("[Install]"));
    assert!(service.contains("ExecStart=/usr/local/bin/spytti"));
    assert!(service.contains("Group=audio"));
    assert!(service.contains("Restart=always"));
}

mod ui {
    const HTML: &str = include_str!("../src/ui.html");

    #[test]
    fn is_valid_html() {
        assert!(HTML.contains("<!DOCTYPE html>"));
        assert!(HTML.contains("<html"));
        assert!(HTML.contains("</html>"));
    }

    #[test]
    fn has_status_elements() {
        assert!(HTML.contains("id=\"track\""));
        assert!(HTML.contains("id=\"artist\""));
        assert!(HTML.contains("id=\"album\""));
        assert!(HTML.contains("id=\"vol\""));
        assert!(HTML.contains("id=\"dot\""));
    }

    #[test]
    fn has_player_controls() {
        assert!(HTML.contains("play-pause"));
        assert!(HTML.contains("next"));
        assert!(HTML.contains("prev"));
    }

    #[test]
    fn uses_svg_icons() {
        assert!(HTML.contains("<svg"));
        assert!(HTML.contains("viewBox"));
        // No emoji icons
        assert!(!HTML.contains("&#9654;&#65039;"));
    }

    #[test]
    fn references_all_api_endpoints() {
        assert!(HTML.contains("/api/status"));
        assert!(HTML.contains("/api/volume"));
        assert!(HTML.contains("/api/devices"));
        assert!(HTML.contains("/api/device"));
        // Player controls go through cmd() which builds '/api/' + action
        assert!(HTML.contains("'/api/'"));
        assert!(HTML.contains("'play-pause'"));
        assert!(HTML.contains("'next'"));
        assert!(HTML.contains("'prev'"));
    }

    #[test]
    fn has_device_selector() {
        assert!(HTML.contains("id=\"device\""));
        assert!(HTML.contains("<select"));
        assert!(HTML.contains("OUTPUT DEVICE"));
    }

    #[test]
    fn has_cover_art() {
        assert!(HTML.contains("id=\"cover\""));
        assert!(HTML.contains("cover_url"));
    }

    #[test]
    fn has_log_viewer() {
        assert!(HTML.contains("/api/logs"));
        assert!(HTML.contains("log-overlay"));
        assert!(HTML.contains("View Logs"));
    }

    #[test]
    fn polls_status_periodically() {
        assert!(HTML.contains("setInterval(poll"));
    }

    #[test]
    fn volume_slider_has_correct_range() {
        assert!(HTML.contains("min=\"0\""));
        assert!(HTML.contains("max=\"100\""));
    }
}
