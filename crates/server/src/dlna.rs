//! Casting to TVs and media players via DLNA/UPnP AV (supported by most smart TVs).
//! Discovery uses SSDP multicast; playback is controlled with AVTransport SOAP calls.
//! The renderer fetches the media itself through a signed, short-lived cast URL.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{Context, anyhow, bail};
use serde::Serialize;
use tokio::net::UdpSocket;

const SSDP_ADDR: &str = "239.255.255.250:1900";
const AV_TRANSPORT: &str = "urn:schemas-upnp-org:service:AVTransport:1";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DlnaDevice {
    pub id: String,
    pub name: String,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub location: String,
    #[serde(skip)]
    pub control_url: String,
}

#[derive(Default)]
pub struct Dlna {
    devices: Mutex<HashMap<String, DlnaDevice>>,
    last_scan: Mutex<Option<Instant>>,
}

pub fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&apos;")
}

fn header<'a>(response: &'a str, name: &str) -> Option<&'a str> {
    response.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        k.trim().eq_ignore_ascii_case(name).then(|| v.trim())
    })
}

/// Parses a UPnP device description and returns the device if it offers AVTransport.
pub fn parse_description(location: &str, xml: &str) -> anyhow::Result<DlnaDevice> {
    let doc = roxmltree::Document::parse(xml).context("invalid device description")?;
    let text = |node: roxmltree::Node, tag: &str| {
        node.descendants().find(|n| n.has_tag_name(tag)).and_then(|n| n.text()).map(|t| t.trim().to_string())
    };
    let root = doc.root_element();
    let base = text(root, "URLBase").unwrap_or_else(|| location.to_string());
    let base = url::Url::parse(&base).or_else(|_| url::Url::parse(location))?;
    let service = root
        .descendants()
        .filter(|n| n.has_tag_name("service"))
        .find(|s| text(*s, "serviceType").map(|t| t.contains("AVTransport")).unwrap_or(false))
        .ok_or_else(|| anyhow!("device has no AVTransport service (not a media renderer)"))?;
    let control = text(service, "controlURL").ok_or_else(|| anyhow!("AVTransport without controlURL"))?;
    // The service belongs to the closest enclosing <device>.
    let device = service.ancestors().find(|n| n.has_tag_name("device")).unwrap_or(root);
    let udn = device
        .children()
        .find(|n| n.has_tag_name("UDN"))
        .and_then(|n| n.text())
        .map(|t| t.trim().to_string())
        .unwrap_or_else(|| location.to_string());
    let child = |tag: &str| device.children().find(|n| n.has_tag_name(tag)).and_then(|n| n.text()).map(|t| t.trim().to_string());
    Ok(DlnaDevice {
        id: udn.trim_start_matches("uuid:").to_string(),
        name: child("friendlyName").unwrap_or_else(|| "Media Renderer".into()),
        manufacturer: child("manufacturer"),
        model: child("modelName"),
        location: location.to_string(),
        control_url: base.join(&control)?.to_string(),
    })
}

/// DIDL-Lite metadata so TVs show title and pick the right player.
pub fn didl(title: &str, url: &str, mime: &str) -> String {
    let class = if mime.starts_with("video/") { "object.item.videoItem" } else { "object.item.imageItem.photo" };
    format!(
        r#"<DIDL-Lite xmlns="urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:upnp="urn:schemas-upnp-org:metadata-1-0/upnp/"><item id="pixelhort" parentID="-1" restricted="1"><dc:title>{}</dc:title><upnp:class>{class}</upnp:class><res protocolInfo="http-get:*:{mime}:*">{}</res></item></DIDL-Lite>"#,
        xml_escape(title),
        xml_escape(url)
    )
}

impl Dlna {
    pub fn devices(&self) -> Vec<DlnaDevice> {
        let mut list: Vec<DlnaDevice> = self.devices.lock().expect("dlna lock").values().cloned().collect();
        list.sort_by(|a, b| a.name.cmp(&b.name));
        list
    }

    pub fn get(&self, id: &str) -> Option<DlnaDevice> {
        self.devices.lock().expect("dlna lock").get(id).cloned()
    }

    pub fn insert(&self, device: DlnaDevice) {
        self.devices.lock().expect("dlna lock").insert(device.id.clone(), device);
    }

    pub fn scanned_recently(&self) -> bool {
        self.last_scan.lock().expect("dlna lock").map(|t| t.elapsed() < Duration::from_secs(60)).unwrap_or(false)
    }

    /// Fetches and registers a renderer by its description URL (for TVs that SSDP cannot reach).
    pub async fn add(&self, http: &reqwest::Client, location: &str) -> anyhow::Result<DlnaDevice> {
        let xml = http.get(location).timeout(Duration::from_secs(5)).send().await?.error_for_status()?.text().await?;
        let device = parse_description(location, &xml)?;
        self.insert(device.clone());
        Ok(device)
    }

    /// Sends SSDP M-SEARCH for media renderers and collects answers for `wait`.
    pub async fn discover(&self, http: &reqwest::Client, wait: Duration) -> anyhow::Result<Vec<DlnaDevice>> {
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        socket.set_multicast_ttl_v4(4)?;
        let search = format!(
            "M-SEARCH * HTTP/1.1\r\nHOST: {SSDP_ADDR}\r\nMAN: \"ssdp:discover\"\r\nMX: 2\r\nST: {AV_TRANSPORT}\r\nUSER-AGENT: Pixelhort/1.0 UPnP/1.1\r\n\r\n"
        );
        for _ in 0..2 {
            socket.send_to(search.as_bytes(), SSDP_ADDR).await?;
        }
        let deadline = Instant::now() + wait;
        let mut locations = Vec::new();
        let mut buf = vec![0u8; 4096];
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            match tokio::time::timeout(left, socket.recv_from(&mut buf)).await {
                Ok(Ok((n, _))) => {
                    let response = String::from_utf8_lossy(&buf[..n]).to_string();
                    if let Some(loc) = header(&response, "LOCATION") {
                        if !locations.iter().any(|l: &String| l == loc) {
                            locations.push(loc.to_string());
                        }
                    }
                }
                _ => break,
            }
        }
        for location in locations {
            if let Err(e) = self.add(http, &location).await {
                tracing::debug!("ignoring UPnP device at {location}: {e:#}");
            }
        }
        *self.last_scan.lock().expect("dlna lock") = Some(Instant::now());
        Ok(self.devices())
    }

    async fn soap(http: &reqwest::Client, device: &DlnaDevice, action: &str, args: &str) -> anyhow::Result<String> {
        let body = format!(
            r#"<?xml version="1.0" encoding="utf-8"?><s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/"><s:Body><u:{action} xmlns:u="{AV_TRANSPORT}">{args}</u:{action}></s:Body></s:Envelope>"#
        );
        // UPnP stacks (e.g. libupnp in many TVs) close connections after each reply, so pooled
        // keep-alive connections go stale: always use a fresh connection and retry once.
        let send = || {
            http.post(&device.control_url)
                .header("Content-Type", r#"text/xml; charset="utf-8""#)
                .header("SOAPACTION", format!("\"{AV_TRANSPORT}#{action}\""))
                .header("Connection", "close")
                .body(body.clone())
                .timeout(Duration::from_secs(10))
                .send()
        };
        let res = match send().await {
            Ok(res) => res,
            Err(e) if e.is_connect() || e.is_request() => {
                tracing::debug!("{action} to {} failed ({e}), retrying", device.name);
                tokio::time::sleep(Duration::from_millis(300)).await;
                send().await.with_context(|| format!("{} is not reachable", device.name))?
            }
            Err(e) => return Err(e).with_context(|| format!("{} is not reachable", device.name)),
        };
        let status = res.status();
        let text = res.text().await.unwrap_or_default();
        if !status.is_success() {
            bail!("{action} failed on {} ({status}): {}", device.name, text.chars().take(300).collect::<String>());
        }
        Ok(text)
    }

    pub async fn play(http: &reqwest::Client, device: &DlnaDevice, url: &str, mime: &str, title: &str) -> anyhow::Result<()> {
        let _ = Self::soap(http, device, "Stop", "<InstanceID>0</InstanceID>").await;
        let args = format!(
            "<InstanceID>0</InstanceID><CurrentURI>{}</CurrentURI><CurrentURIMetaData>{}</CurrentURIMetaData>",
            xml_escape(url),
            xml_escape(&didl(title, url, mime))
        );
        Self::soap(http, device, "SetAVTransportURI", &args).await?;
        Self::soap(http, device, "Play", "<InstanceID>0</InstanceID><Speed>1</Speed>").await?;
        Ok(())
    }

    pub async fn stop(http: &reqwest::Client, device: &DlnaDevice) -> anyhow::Result<()> {
        Self::soap(http, device, "Stop", "<InstanceID>0</InstanceID>").await.map(|_| ())
    }

    /// Returns the renderer's transport state (PLAYING, STOPPED, TRANSITIONING, …).
    pub async fn state(http: &reqwest::Client, device: &DlnaDevice) -> anyhow::Result<String> {
        let xml = Self::soap(http, device, "GetTransportInfo", "<InstanceID>0</InstanceID>").await?;
        let doc = roxmltree::Document::parse(&xml)?;
        doc.descendants()
            .find(|n| n.has_tag_name("CurrentTransportState"))
            .and_then(|n| n.text())
            .map(|s| s.trim().to_string())
            .ok_or_else(|| anyhow!("no transport state in response"))
    }

    /// The address under which `device` can reach this server (local IP of the outgoing route).
    pub fn local_ip_towards(device: &DlnaDevice) -> Option<IpAddr> {
        let url = url::Url::parse(&device.location).ok()?;
        let host = url.host_str()?;
        let port = url.port_or_known_default()?;
        let target: SocketAddr = format!("{host}:{port}").parse().ok()?;
        let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
        socket.connect(target).ok()?;
        socket.local_addr().ok().map(|a| a.ip())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DESCRIPTION: &str = r#"<?xml version="1.0"?>
<root xmlns="urn:schemas-upnp-org:device-1-0">
  <device>
    <deviceType>urn:schemas-upnp-org:device:MediaRenderer:1</deviceType>
    <friendlyName>Wohnzimmer-TV</friendlyName>
    <manufacturer>ACME</manufacturer>
    <modelName>Vision 55</modelName>
    <UDN>uuid:1234-abcd</UDN>
    <serviceList>
      <service><serviceType>urn:schemas-upnp-org:service:RenderingControl:1</serviceType><controlURL>/rc</controlURL></service>
      <service><serviceType>urn:schemas-upnp-org:service:AVTransport:1</serviceType><controlURL>/upnp/control/avt</controlURL></service>
    </serviceList>
  </device>
</root>"#;

    #[test]
    fn parses_renderer_description() {
        let d = parse_description("http://192.168.1.20:49152/description.xml", DESCRIPTION).unwrap();
        assert_eq!(d.id, "1234-abcd");
        assert_eq!(d.name, "Wohnzimmer-TV");
        assert_eq!(d.model.as_deref(), Some("Vision 55"));
        assert_eq!(d.control_url, "http://192.168.1.20:49152/upnp/control/avt");
    }

    #[test]
    fn rejects_non_renderers() {
        let xml = DESCRIPTION.replace("AVTransport", "ContentDirectory");
        assert!(parse_description("http://x/d.xml", &xml).is_err());
    }

    #[test]
    fn didl_escapes_urls() {
        let d = didl("A & B", "http://h/v?a=1&b=2", "video/mp4");
        assert!(d.contains("A &amp; B"));
        assert!(d.contains("a=1&amp;b=2"));
        assert!(d.contains("object.item.videoItem"));
    }

    #[test]
    fn reads_ssdp_headers() {
        let r = "HTTP/1.1 200 OK\r\nLocation: http://10.0.0.5:49152/description.xml\r\nST: x\r\n\r\n";
        assert_eq!(header(r, "LOCATION"), Some("http://10.0.0.5:49152/description.xml"));
    }
}
