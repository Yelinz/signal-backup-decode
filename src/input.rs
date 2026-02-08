use anyhow::anyhow;
use anyhow::Context;
use byteorder::ReadBytesExt;
use log::{debug, info};
use std::convert::TryInto;
use std::io::Read;

/// Read input file
pub struct InputFile {
	reader: std::io::BufReader<std::fs::File>,
	decrypter: crate::decrypter::Decrypter,
	count_frame: usize,
	count_byte: usize,
	file_bytes: u64,
}

impl InputFile {
	pub fn new(
		path: &std::path::Path,
		password: &[u8],
		verify_mac: bool,
	) -> Result<Self, anyhow::Error> {
		// open file
		info!("Input file: {}", &path.to_string_lossy());
		let file = std::fs::File::open(path)
			.with_context(|| format!("Could not open backup file: {}", path.to_string_lossy()))?;
		let file_bytes = file.metadata().unwrap().len();
		let mut reader = std::io::BufReader::new(file);

		// create decrypter
		// - read first frame
		let len: usize = reader
			.read_u32::<byteorder::BigEndian>()
			.context("Failed to read frame length from backup file")?
			.try_into()
			.context("Frame length too large to fit in memory")?;
		let mut frame = vec![0u8; len];
		reader.read_exact(&mut frame)?;
		let frame: crate::frame::Frame = frame.try_into()?;
		debug!("Frame type: {}", &frame);

		// check that frame is a header and return
		match &frame {
			crate::frame::Frame::Header { salt, iv } => Ok(Self {
				reader,
				decrypter: crate::decrypter::Decrypter::new(&password, &salt, &iv, verify_mac),
				count_frame: 1,
				// We already read `len` and 4 bytes with read_u32
				// There are 16 bytes missing somewhere independent of the input
				// file. However, I don't know why.
				count_byte: len + std::mem::size_of::<u32>() + 16,
				file_bytes,
			}),
			_ => Err(anyhow!("first frame is not a header")),
		}
	}

	fn read_data(
		&mut self,
		length: usize,
		read_attachment: bool,
	) -> Result<Vec<u8>, anyhow::Error> {
		let mut hmac = [0u8; crate::decrypter::LENGTH_HMAC];
		let mut data;

		// Reading files (attachments) need an update of MAC with IV.
		// And their given length corresponds to file length but frame length corresponds
		// to data length + hmac data.
		if read_attachment {
			self.decrypter.mac_update_with_iv();
			data = vec![0u8; length];
		} else {
			data = vec![0u8; length - crate::decrypter::LENGTH_HMAC];
		}

		// read data and decrypt
		self.reader.read_exact(&mut data)?;
		let data = self.decrypter.decrypt(&mut data)?;

		// read hmac
		self.reader.read_exact(&mut hmac)?;

		// verify mac
		self.decrypter.verify_mac(&hmac)?;
		self.decrypter.increase_iv();

		if read_attachment {
			// we got file length, so we have to add 10 bytes for hmac data
			self.count_byte += length + crate::decrypter::LENGTH_HMAC;
		} else {
			// in the case of frames, we add 4 bytes we have read to determine frame length
			// (hmac data is already in length included)
			self.count_byte += length + std::mem::size_of::<u32>();
		}

		Ok(data)
	}

	pub fn read_frame(&mut self) -> Result<crate::frame::Frame, anyhow::Error> {
		// Read frame length (4 encrypted bytes)
		let mut frame_len_bytes = [0u8; 4];
		self.reader.read_exact(&mut frame_len_bytes)
			.context("Failed to read frame length from backup file")?;
		
		debug!(
			"Raw encrypted frame length bytes for frame {}: {:02X?}",
			self.count_frame + 1,
			frame_len_bytes
		);
		
		// Preview decrypt the length WITHOUT updating HMAC
		// We use openssl directly to avoid HMAC side effects
		let decrypted_len_bytes = openssl::symm::decrypt(
			openssl::symm::Cipher::aes_256_ctr(),
			self.decrypter.get_key(),
			Some(self.decrypter.get_iv()),
			&frame_len_bytes,
		).map_err(|e| anyhow!("Failed to decrypt frame length: {}", e))?;
		
		let frame_len_raw = u32::from_be_bytes([
			decrypted_len_bytes[0],
			decrypted_len_bytes[1],
			decrypted_len_bytes[2],
			decrypted_len_bytes[3],
		]);
		
		debug!(
			"Decrypted frame length for frame {}: {} bytes (0x{:08X})",
			self.count_frame + 1,
			frame_len_raw,
			frame_len_raw
		);
		
		let len: usize = frame_len_raw
			.try_into()
			.context(format!("Frame length {} is too large to fit in memory", frame_len_raw))?;
		
		// Validate frame length is reasonable (max 100MB per frame)
		const MAX_FRAME_SIZE: usize = 100 * 1024 * 1024;
		if len > MAX_FRAME_SIZE {
			return Err(anyhow!(
				"Frame {} has unreasonably large length of {} bytes (max {} bytes). This likely indicates a corrupted backup file or incorrect password.",
				self.count_frame + 1,
				len,
				MAX_FRAME_SIZE
			));
		}
		
		debug!(
			"Reading frame {} with length of {} bytes",
			self.count_frame + 1, len
		);

		// len includes the 10-byte HMAC, so actual encrypted data is len - 10
		let data_len = len.checked_sub(crate::decrypter::LENGTH_HMAC)
			.ok_or_else(|| anyhow!("Frame length {} is too small to contain HMAC", len))?;
		
		// Read the encrypted frame data
		let mut encrypted_data = vec![0u8; data_len];
		self.reader.read_exact(&mut encrypted_data)?;
		
		// Concatenate length + data and decrypt as ONE continuous stream
		// This is crucial for CTR mode to work correctly
		let mut all_encrypted = Vec::with_capacity(4 + data_len);
		all_encrypted.extend_from_slice(&frame_len_bytes);
		all_encrypted.extend_from_slice(&encrypted_data);
		
		// Decrypt everything together (length + data) - this also updates HMAC
		let all_decrypted = self.decrypter.decrypt(&all_encrypted)?;
		
		// Extract just the frame data part (skip the 4-byte length prefix)
		let data = all_decrypted[4..].to_vec();
		
		// Read and verify HMAC
		let mut hmac = [0u8; crate::decrypter::LENGTH_HMAC];
		self.reader.read_exact(&mut hmac)?;
		self.decrypter.verify_mac(&hmac)?;
		
		// Increment IV for next frame
		self.decrypter.increase_iv();
		
		// Update byte counter (4 bytes length + len bytes for data+hmac)
		self.count_byte += 4 + len;

		// Parse frame from decrypted data
		let mut frame: crate::frame::Frame = data.try_into()?;
		debug!("Frame type: {}", &frame);

		match frame {
			crate::frame::Frame::Attachment { data_length, .. } => {
				frame.set_data(self.read_data(data_length, true)?);
			}
			crate::frame::Frame::Avatar { data_length, .. } => {
				frame.set_data(self.read_data(data_length, true)?);
			}
			crate::frame::Frame::Sticker { data_length, .. } => {
				frame.set_data(self.read_data(data_length, true)?);
			}
			crate::frame::Frame::Header { .. } => return Err(anyhow!("unexpected header found")),
			_ => (),
		};

		// clean up and return
		self.count_frame += 1;
		Ok(frame)
	}

	pub fn get_count_frame(&self) -> usize {
		self.count_frame
	}

	pub fn get_count_byte(&self) -> usize {
		self.count_byte
	}

	pub fn get_file_size(&self) -> u64 {
		self.file_bytes
	}
}

impl Iterator for InputFile {
	type Item = Result<crate::frame::Frame, anyhow::Error>;

	fn next(&mut self) -> Option<Self::Item> {
		let ret = self.read_frame();

		if let Ok(x) = ret {
			match x {
				crate::frame::Frame::End => None,
				_ => Some(Ok(x)),
			}
		} else {
			Some(ret)
		}
	}
}
