//! Offline extraction of checked KV images into newly created dense CPU files.
//! No model execution, live authority, authentication or crash-atomic publish.

use fa_reference::action::consequence::activation::tensor::{BufferIdentity, TensorContract, TensorLayout};
use fa_reference::action::consequence::activation::tensor::kv::image::{KvImage, KvImageDescriptor, IMAGE_HEADER_BYTES, MAX_IMAGE_BYTES};
use fa_reference::action::consequence::activation::tensor::kv::restore::{HostTensorMut, KvDestination, KvRestoreWindow};
use fa_reference::Error;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

fn model<T>(result: Result<T, Error>) -> Result<T, String> { result.map_err(|error| format!("image refused: {error:?}")) }

fn bounded_read(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    if !file.metadata().map_err(|error| error.to_string())?.is_file() { return Err("input must be a regular file".into()); }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut bytes).map_err(|error| error.to_string())?;
    if bytes.len() > limit { return Err(format!("input exceeds {limit} bytes")); }
    Ok(bytes)
}

fn dense(contract: &TensorContract, tokens: usize) -> Result<TensorLayout, String> {
    let width = contract.encoding().bytes();
    model(TensorLayout::new([1, tokens, contract.heads(), contract.channels()],
        [0, contract.dimensions() * width, contract.channels() * width, width], 0,
        contract.encoding(), contract.byte_order()))
}

fn restore_buffers(image: &KvImage) -> Result<(Vec<u8>, Vec<u8>), String> {
    if image.is_empty() { return Err("an empty observed prefix has no values to restore".into()); }
    let descriptor = image.descriptor();
    let kl = dense(descriptor.contract.keys(), image.len())?;
    let vl = dense(descriptor.contract.values(), image.len())?;
    let mut keys = vec![0; kl.byte_range().end];
    let mut values = vec![0; vl.byte_range().end];
    model(image.prepare_restore(KvDestination::Separate {
        keys: HostTensorMut { identity: BufferIdentity { object: 1, generation: 1 }, layout: &kl, bytes: &mut keys },
        values: HostTensorMut { identity: BufferIdentity { object: 2, generation: 1 }, layout: &vl, bytes: &mut values },
    }, KvRestoreWindow { first_position: descriptor.first_position, token_count: image.len(),
        batch: 0, first_token: 0, buffer_first_position: descriptor.first_position }))?.commit();
    Ok((keys, values))
}

fn create_output(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(|error| format!("create {}: {error}", path.display()))?;
    file.write_all(bytes).map_err(|error| format!("write {}: {error}", path.display()))
}

fn run(manifest: &Path, image: &Path, output: &Path) -> Result<(), String> {
    let descriptor_bytes = bounded_read(manifest, IMAGE_HEADER_BYTES)?;
    let descriptor = model(KvImageDescriptor::decode(&descriptor_bytes))?;
    let bytes = bounded_read(image, MAX_IMAGE_BYTES)?;
    let image = model(KvImage::decode(&bytes, &descriptor))?;
    let (keys, values) = restore_buffers(&image)?;
    let k = descriptor.contract.keys();
    let v = descriptor.contract.values();
    let layout = format!(
        "scope=cpu_cache_values_only\nauthentication=not_established\nrestart_grade=not_established\n\
         axes=batch,token,cache_head,channel\nphysical_order=contiguous_row_major\n\
         batch=0\npositions={}\nfirst_position={}\nsource_stream={}\nfirst_sequence={}\n\
         source_batch={}\nsource_revision={}\nquery_heads={}\n\
         keys_shape=1,{},{},{}\nkeys_encoding={:?}\nkeys_byte_order={:?}\n\
         values_shape=1,{},{},{}\nvalues_encoding={:?}\nvalues_byte_order={:?}\n",
        image.len(), descriptor.first_position, descriptor.stream, descriptor.first_sequence,
        descriptor.source_batch, descriptor.source_revision, descriptor.contract.query_heads(),
        image.len(), k.heads(), k.channels(), k.encoding(), k.byte_order(),
        image.len(), v.heads(), v.channels(), v.encoding(), v.byte_order(),
    );
    // Decode and RAM restoration have already succeeded. Filesystem output is
    // deliberately separate and may be partial on I/O failure; never overwrite.
    let mut directory = fs::DirBuilder::new();
    directory.recursive(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        directory.mode(0o700);
    }
    directory.create(output).map_err(|error| format!("create new output directory {}: {error}", output.display()))?;
    create_output(&output.join("keys.bin"), &keys)?;
    create_output(&output.join("values.bin"), &values)?;
    create_output(&output.join("descriptor.bin"), &descriptor_bytes)?;
    create_output(&output.join("layout.txt"), layout.as_bytes())?;
    println!("restored_positions={} key_bytes={} value_bytes={} scope=cpu_cache_values_only", image.len(), keys.len(), values.len());
    Ok(())
}

fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() == 1 && args[0] == "--help" {
        println!("Usage: kv_image_restore <independent-descriptor.bin> <image.bin> <new-output-directory>\n\
            Restores finite cache values only. Authenticate inputs separately. Does not resume a model.");
        return;
    }
    if args.len() != 3 {
        eprintln!("Usage: kv_image_restore <independent-descriptor.bin> <image.bin> <new-output-directory>");
        std::process::exit(2);
    }
    let result = run(&PathBuf::from(&args[0]), &PathBuf::from(&args[1]), &PathBuf::from(&args[2]));
    if let Err(error) = result {
        eprintln!("{error}; any newly created output directory may contain partial files");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fa_reference::action::consequence::activation::CaptureProfile;
    use fa_reference::action::consequence::activation::tensor::{ByteOrder, HostTensor, ScalarEncoding};
    use fa_reference::action::consequence::activation::tensor::kv::{KvAppend, KvBudget, KvCapture, KvContract};

    fn captured() -> KvImage {
        let profile = CaptureProfile { tenant: 1, model: 2, model_generation: 3, tap: 4, layout_generation: 5 };
        let k = TensorContract::new(profile, ScalarEncoding::Binary32, ByteOrder::Little, 1, 1).unwrap();
        let v = TensorContract::new(CaptureProfile { tap: 6, ..profile }, ScalarEncoding::Binary16, ByteOrder::Big, 1, 1).unwrap();
        let kl = dense(&k, 1).unwrap();
        let vl = dense(&v, 1).unwrap();
        let mut c = KvCapture::new(KvContract::new(k, v, 2).unwrap(), 7, 0, 10, 20,
            KvBudget { positions: 1, normalized_values: 2 }).unwrap();
        c.append(0, KvAppend {
            keys: HostTensor { identity: BufferIdentity { object: 1, generation: 1 }, layout: &kl, bytes: &(-0_f32).to_le_bytes() },
            values: HostTensor { identity: BufferIdentity { object: 2, generation: 1 }, layout: &vl, bytes: &0x3c00_u16.to_be_bytes() },
            first_token: 0, token_count: 1, buffer_first_position: 10, first_sequence: 20,
        }).unwrap();
        c.snapshot(1).unwrap()
    }

    #[test]
    fn file_consumer_path_decodes_and_restores_exact_dense_scalar_bytes() {
        let original = captured();
        let descriptor = KvImageDescriptor::decode(&original.descriptor().encode().unwrap()).unwrap();
        let image = KvImage::decode(&original.encode().unwrap(), &descriptor).unwrap();
        let (keys, values) = restore_buffers(&image).unwrap();
        assert_eq!(keys, (-0_f32).to_le_bytes());
        assert_eq!(values, 0x3c00_u16.to_be_bytes());
    }

    #[test]
    fn file_consumer_layout_rejects_an_empty_destination() {
        assert!(dense(captured().descriptor().contract.keys(), 0).is_err());
    }

    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            for _ in 0..1000 {
                let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let path = std::env::temp_dir().join(format!("fa-kv-restore-{}-{id}", std::process::id()));
                match fs::create_dir(&path) {
                    Ok(()) => return Self(path),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(error) => panic!("temporary directory: {error}"),
                }
            }
            panic!("temporary directory collision limit")
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) { fs::remove_dir_all(&self.0).expect("remove only this test's owned directory"); }
    }

    #[test]
    fn restores_files_from_disk_and_never_overwrites_an_existing_output_directory() {
        let temporary = TempDir::new();
        let original = captured();
        let manifest = temporary.0.join("manifest.bin");
        let image = temporary.0.join("image.bin");
        let output = temporary.0.join("restored");
        fs::write(&manifest, original.descriptor().encode().unwrap()).unwrap();
        fs::write(&image, original.encode().unwrap()).unwrap();
        run(&manifest, &image, &output).unwrap();
        assert_eq!(fs::read(output.join("keys.bin")).unwrap(), (-0_f32).to_le_bytes());
        assert_eq!(fs::read(output.join("values.bin")).unwrap(), 0x3c00_u16.to_be_bytes());
        assert!(fs::read_to_string(output.join("layout.txt")).unwrap().contains("restart_grade=not_established"));
        fs::write(output.join("keys.bin"), b"do not overwrite").unwrap();
        assert!(run(&manifest, &image, &output).is_err());
        assert_eq!(fs::read(output.join("keys.bin")).unwrap(), b"do not overwrite");
    }

    #[test]
    fn malformed_disk_input_creates_no_output_and_bounded_reads_reject_extra_bytes() {
        let temporary = TempDir::new();
        let original = captured();
        let manifest = temporary.0.join("manifest.bin");
        let image = temporary.0.join("image.bin");
        let output = temporary.0.join("refused");
        fs::write(&manifest, original.descriptor().encode().unwrap()).unwrap();
        let mut bytes = original.encode().unwrap();
        bytes.pop();
        fs::write(&image, bytes).unwrap();
        assert!(run(&manifest, &image, &output).is_err());
        assert!(!output.exists());
        assert!(bounded_read(&manifest, IMAGE_HEADER_BYTES - 1).is_err());
    }

}
