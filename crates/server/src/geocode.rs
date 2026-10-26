//! Offline reverse geocoding using the GeoNames `cities500` dataset
//! (downloaded once into `<data>/geodata`).

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use std::sync::{Arc, RwLock};

use anyhow::Context;

const BASE: &str = "https://download.geonames.org/export/dump";

#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    pub city: String,
    pub state: Option<String>,
    pub country: Option<String>,
}

struct City {
    lat: f64,
    lon: f64,
    name: String,
    country_code: String,
    admin1: String,
}

pub struct GeoIndex {
    cities: Vec<City>,
    grid: HashMap<(i32, i32), Vec<u32>>,
    admin1: HashMap<String, String>,
    countries: HashMap<String, String>,
}

#[derive(Default)]
pub struct Geocoder {
    index: RwLock<Option<Arc<GeoIndex>>>,
}

fn cell(lat: f64, lon: f64) -> (i32, i32) {
    (lat.floor() as i32, lon.floor() as i32)
}

impl GeoIndex {
    pub fn parse(cities_tsv: &str, admin1_tsv: &str, country_tsv: &str) -> Self {
        let mut cities = Vec::new();
        let mut grid: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
        for line in cities_tsv.lines() {
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() < 11 {
                continue;
            }
            // Skip city sections, historical, abandoned and destroyed places.
            if matches!(f[7], "PPLX" | "PPLH" | "PPLQ" | "PPLW") {
                continue;
            }
            let (Ok(lat), Ok(lon)) = (f[4].parse::<f64>(), f[5].parse::<f64>()) else { continue };
            grid.entry(cell(lat, lon)).or_default().push(cities.len() as u32);
            cities.push(City {
                lat,
                lon,
                name: f[1].to_string(),
                country_code: f[8].to_string(),
                admin1: f[10].to_string(),
            });
        }
        let admin1 = admin1_tsv
            .lines()
            .filter_map(|l| {
                let mut it = l.split('\t');
                Some((it.next()?.to_string(), it.next()?.to_string()))
            })
            .collect();
        let countries = country_tsv
            .lines()
            .filter(|l| !l.starts_with('#'))
            .filter_map(|l| {
                let f: Vec<&str> = l.split('\t').collect();
                (f.len() > 4).then(|| (f[0].to_string(), f[4].to_string()))
            })
            .collect();
        Self { cities, grid, admin1, countries }
    }

    pub fn lookup(&self, lat: f64, lon: f64) -> Option<Place> {
        let (cy, cx) = cell(lat, lon);
        let coslat = lat.to_radians().cos();
        let mut best: Option<(f64, &City)> = None;
        for dy in -1..=1 {
            for dx in -1..=1 {
                let mut x = cx + dx;
                if x < -180 {
                    x += 360;
                } else if x >= 180 {
                    x -= 360;
                }
                for &i in self.grid.get(&(cy + dy, x)).into_iter().flatten() {
                    let c = &self.cities[i as usize];
                    let mut dlon = (c.lon - lon).abs();
                    if dlon > 180.0 {
                        dlon = 360.0 - dlon;
                    }
                    let d = (c.lat - lat).powi(2) + (dlon * coslat).powi(2);
                    if best.map(|(bd, _)| d < bd).unwrap_or(true) {
                        best = Some((d, c));
                    }
                }
            }
        }
        let (_, c) = best?;
        Some(Place {
            city: c.name.clone(),
            state: self.admin1.get(&format!("{}.{}", c.country_code, c.admin1)).cloned(),
            country: self.countries.get(&c.country_code).cloned(),
        })
    }
}

impl Geocoder {
    pub fn lookup(&self, lat: f64, lon: f64) -> Option<Place> {
        self.index.read().ok()?.as_ref()?.lookup(lat, lon)
    }

    pub fn is_loaded(&self) -> bool {
        self.index.read().map(|i| i.is_some()).unwrap_or(false)
    }

    /// Loads the dataset (downloading it on first use).
    pub async fn load(&self, http: &reqwest::Client, dir: &Path) -> anyhow::Result<()> {
        tokio::fs::create_dir_all(dir).await?;
        let files = [("cities500.zip", "cities500.zip"), ("admin1CodesASCII.txt", "admin1.txt"), ("countryInfo.txt", "countries.txt")];
        for (remote, local) in files {
            let path = dir.join(local);
            if !path.exists() {
                tracing::info!("downloading GeoNames {remote}");
                let bytes = http
                    .get(format!("{BASE}/{remote}"))
                    .send()
                    .await?
                    .error_for_status()?
                    .bytes()
                    .await?;
                let tmp = path.with_extension("part");
                tokio::fs::write(&tmp, &bytes).await?;
                tokio::fs::rename(&tmp, &path).await?;
            }
        }
        let dir = dir.to_path_buf();
        let index = tokio::task::spawn_blocking(move || -> anyhow::Result<GeoIndex> {
            let zip = std::fs::File::open(dir.join("cities500.zip"))?;
            let mut archive = zip::ZipArchive::new(zip)?;
            let mut cities = String::new();
            archive.by_name("cities500.txt").context("cities500.txt missing")?.read_to_string(&mut cities)?;
            let admin1 = std::fs::read_to_string(dir.join("admin1.txt"))?;
            let countries = std::fs::read_to_string(dir.join("countries.txt"))?;
            Ok(GeoIndex::parse(&cities, &admin1, &countries))
        })
        .await??;
        tracing::info!("reverse geocoding ready ({} places)", index.cities.len());
        *self.index.write().expect("geocoder lock") = Some(Arc::new(index));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_city() {
        let cities = "1\tBerlin\tBerlin\t\t52.52437\t13.41053\tP\tPPLC\tDE\t\t16\t\t\t\t3426354\n\
                      2\tPotsdam\tPotsdam\t\t52.39886\t13.06566\tP\tPPLA\tDE\t\t11\t\t\t\t178089\n\
                      3\tTiergarten\tTiergarten\t\t52.51\t13.37\tP\tPPLX\tDE\t\t16\t\t\t\t12000\n";
        let admin = "DE.16\tBerlin\tBerlin\t1\nDE.11\tBrandenburg\tBrandenburg\t2\n";
        let countries = "#ISO\tISO3\tISO-Numeric\tfips\tCountry\nDE\tDEU\t276\tGM\tGermany\n";
        let idx = GeoIndex::parse(cities, admin, countries);
        let p = idx.lookup(52.41, 13.1).unwrap();
        assert_eq!(p.city, "Potsdam");
        assert_eq!(p.state.as_deref(), Some("Brandenburg"));
        assert_eq!(p.country.as_deref(), Some("Germany"));
        assert_eq!(idx.lookup(52.5163, 13.3777).unwrap().city, "Berlin");
        assert!(idx.lookup(-33.0, 151.0).is_none());
    }
}
