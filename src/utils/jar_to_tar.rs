use crate::utils::fx_hash::FxBuildHasher;
use flate2::Compression;
use flate2::GzBuilder;
use hashbrown::HashMap;
use jay_algorithms::jar::JarError;
use jay_algorithms::jar::JarEvent;
use jay_algorithms::jar::JarReader;
use jay_algorithms::tar::TarWriter;
use std::io;
use std::io::BufWriter;
use thiserror::Error;
use uapi::OwnedFd;

#[derive(Debug, Error)]
pub enum JarToTarError {
    #[error("Could not create event reader")]
    OpenJar(#[source] JarError),
    #[error("Could not read event")]
    ReadJar(#[source] JarError),
    #[error("Could not write tar")]
    WriteTar(#[source] io::Error),
    #[error("Jar refers to an unknown link")]
    MissingLink,
}

#[expect(unused)]
pub fn jar_to_tar(root: &str, src: &OwnedFd, dst: &OwnedFd) -> Result<(), JarToTarError> {
    let mut reader = JarReader::new(src).map_err(JarToTarError::OpenJar)?;
    let filename = format!("{root}.tar");
    let writer = BufWriter::with_capacity(1 << 17, dst.borrow());
    let writer = GzBuilder::new()
        .filename(filename)
        .write(writer, Compression::default());
    let mut buf_writer = BufWriter::new(writer);
    let mut tar_writer = TarWriter::new(&mut buf_writer);
    let mut lens = vec![];
    let mut path = vec![];
    let mut paths = HashMap::with_hasher(FxBuildHasher);
    loop {
        let ev = reader.next().map_err(JarToTarError::ReadJar)?;
        let Some(ev) = ev else {
            break;
        };
        let len = path.len();
        let res = match ev {
            JarEvent::Dir(p) => {
                lens.push(len);
                path.extend_from_slice(p);
                path.push(b'/');
                tar_writer.add_dir(&path)
            }
            JarEvent::DirUp => {
                let len = lens.pop().unwrap();
                path.truncate(len);
                Ok(())
            }
            JarEvent::Reg(p, unique, c) => {
                path.extend_from_slice(p);
                if let Some(unique) = unique {
                    paths.insert(unique, path.clone());
                }
                let res = tar_writer.add_reg(&path, c);
                path.truncate(len);
                res
            }
            JarEvent::Lnk(p, l) => {
                path.extend_from_slice(p);
                let res = tar_writer.add_lnk(&path, l);
                path.truncate(len);
                res
            }
            JarEvent::Hrd(p, unique) => {
                let l = paths.get(&unique).ok_or(JarToTarError::MissingLink)?;
                path.extend_from_slice(p);
                let res = tar_writer.add_hrd(&path, l);
                path.truncate(len);
                res
            }
        };
        res.map_err(JarToTarError::WriteTar)?;
    }
    tar_writer.finish().map_err(JarToTarError::WriteTar)?;
    buf_writer
        .into_inner()
        .map_err(|e| e.into_error())
        .map_err(JarToTarError::WriteTar)?
        .finish()
        .map_err(JarToTarError::WriteTar)?
        .into_inner()
        .map_err(|e| e.into_error())
        .map_err(JarToTarError::WriteTar)?;
    Ok(())
}
