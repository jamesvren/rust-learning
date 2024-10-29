use serde::{Serialize, Deserialize};
use csv::{ReaderBuilder, WriterBuilder};
use clap::Parser;

use std::fs;
use std::error::Error;

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Record {
    cves: String,
    description: String,
    feed_rating: String,
    fixed_version: String,
    in_base_image: String,
    last_modified_timestamp: String,
    link: String,
    name: String,
    package_name: String,
    package_version: String,
    published_timestamp: String,
    score: String,
    score_v3: String,
    severity: String,
    vectors: String,
    vectors_v3: String,
    tags: String,
}

#[derive(Debug, PartialEq, Serialize)]
struct Cve {
    name: String,
    severity: String,
    package: String,
    version: String,
    fixed_version: String,
    published_timestamp: String,
    image: String,
    fixed: bool,
    wontfix_reasion: String,
    description: String,
}

impl From<Record> for Cve {
    fn from(r: Record) -> Self {
        Self {
            name: r.name,
            severity: r.severity,
            package: r.package_name,
            version: r.package_version,
            fixed_version: r.fixed_version,
            published_timestamp: r.published_timestamp,
            image: String::new(),
            fixed: false,
            wontfix_reasion: String::new(),
            description: r.description,
        }
    }
}


#[derive(Parser)]
struct Opt {
    #[arg(long, short)]
    dir: String,
}

fn merge_csv(csv: &str, cves: &mut Vec<Cve>) -> Result<(), Box<dyn Error>>{
    let mut rdr = ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_path(csv)?;

    let mut de = rdr.deserialize();
    let image: Record = de.next().ok_or("failed to parse raw")??;
    let image: Vec<_> = image.cves.split('|').collect();
    // skip sencond record which is actrually a header
    let _ = de.next();
    for result in de {
        let record: Record = result?;
        let mut cve = Cve::from(record);
        cve.image = image[0].to_string();
        cves.push(cve);
    }
    Ok(())
}

const CVES: &str = "cves.csv";

fn main() -> Result<(), Box<dyn Error>>{
    let opt = Opt::parse();
    let mut cves: Vec<Cve> = Vec::new();

    // Merge csv file to record
    for entry in fs::read_dir(&opt.dir)? {
        let dir = entry?;
        if dir.path().ends_with(CVES) {
            continue;
        }
        match dir.path().extension() {
            Some(ext) if ext == "csv" => {
                let res = merge_csv(dir.path().to_str().unwrap(), &mut cves);
                println!("{:?}", res);
            },
            _ => continue,
        }
        println!("{:?}", dir.path());
    }

    // Write to the file
    let mut wtr = WriterBuilder::new().from_path(CVES)?;
    cves.dedup();
    for cve in cves {
        wtr.serialize(cve)?;
    }
    wtr.flush()?;

    Ok(())
}
