use anyhow::Context;
use std::convert::TryInto;

/// Frame
pub enum Frame {
	Header {
		salt: Vec<u8>,
		iv: Vec<u8>,
	},
	Statement {
		statement: String,
		parameter: Vec<rusqlite::types::Value>,
	},
	Preference {
		preference: crate::Backups::SharedPreference,
	},
	Attachment {
		data_length: usize,
		id: u64,
		row: u64,
		data: Option<Vec<u8>>,
	},
	Version {
		version: u32,
	},
	End,
	Avatar {
		data_length: usize,
		name: String,
		data: Option<Vec<u8>>,
	},
	Sticker {
		data_length: usize,
		row: u64,
		data: Option<Vec<u8>>,
	},
	KeyValue {
		key_value: crate::Backups::KeyValue
		    // optional string key          = 1;
    // optional bytes  blobValue    = 2;
    // optional bool   booleanValue = 3;
    // optional float  floatValue   = 4;
    // optional int32  integerValue = 5;
    // optional int64  longValue    = 6;
    // optional string stringValue  = 7;
	}
}

impl Frame {
	pub fn new(frame: &mut crate::Backups::BackupFrame) -> Self {
		let mut fields_count = 0;
		let mut ret: Option<Self> = None;

		if frame.header.is_some() {
			fields_count += 1;
			let header = frame.header.take().unwrap();
			ret = Some(Self::Header {
				salt: header.salt.unwrap_or_default(),
				iv: header.iv.unwrap_or_default(),
			});
		};

		if frame.statement.is_some() {
			fields_count += 1;
			let statement = frame.statement.take().unwrap();
			ret = Some(Self::Statement {
				statement: statement.statement.clone().unwrap_or_default(),
				parameter: {
					let mut params: Vec<rusqlite::types::Value> = Vec::new();
					for param in statement.parameters.iter() {
						if param.has_stringParamter() {
							params.push(param.stringParamter().to_string().into());
						} else if param.has_integerParameter() {
							params.push((param.integerParameter() as i64).into());
						} else if param.has_doubleParameter() {
							params.push(param.doubleParameter().into());
						} else if param.has_blobParameter() {
							params.push(param.blobParameter().to_vec().into());
						} else if param.has_nullparameter() {
							params.push(rusqlite::types::Null.into());
						} else {
							panic!("Parameter type not known {:?}", param);
						}
					}
					params
				},
			});
		};

		if frame.preference.is_some() {
			fields_count += 1;
			ret = Some(Self::Preference {
				preference: frame.preference.take().unwrap(),
			});
		};

		if frame.attachment.is_some() {
			fields_count += 1;
			let attachment = frame.attachment.as_ref().unwrap();
			ret = Some(Self::Attachment {
				data_length: attachment.length.unwrap_or(0).try_into().unwrap(),
				id: attachment.attachmentId.unwrap_or(0),
				row: attachment.rowId.unwrap_or(0),
				data: None,
			});
		};

		if frame.version.is_some() {
			fields_count += 1;
			let version = frame.version.as_ref().unwrap();
			ret = Some(Self::Version {
				version: version.version.unwrap_or(0),
			});
		};

		if frame.has_end() {
			fields_count += 1;
			ret = Some(Self::End);
		};

		if frame.avatar.is_some() {
			fields_count += 1;
			let avatar = frame.avatar.as_ref().unwrap();
			ret = Some(Self::Avatar {
				data_length: avatar.length.unwrap_or(0).try_into().unwrap(),
				name: avatar.name.clone().unwrap_or_default(),
				data: None,
			});
		};

		if frame.sticker.is_some() {
			fields_count += 1;
			let sticker = frame.sticker.as_ref().unwrap();
			ret = Some(Self::Sticker {
				data_length: sticker.length.unwrap_or(0).try_into().unwrap(),
				row: sticker.rowId.unwrap_or(0),
				data: None,
			});
		};

		if frame.keyValue.is_some() {
			fields_count += 1;
			let key_value = frame.keyValue.take().unwrap();
			ret = Some(Self::KeyValue {
				key_value
			});
		};

		if fields_count != 1 {
			panic!(
				"Frame with an unsupported number of fields found, please report to author: {:?}",
				frame
			);
		};

		ret.unwrap()
	}

	pub fn set_data(&mut self, data_add: Vec<u8>) {
		match self {
			Frame::Attachment { data, .. } => *data = Some(data_add),
			Frame::Avatar { data, .. } => *data = Some(data_add),
			Frame::Sticker { data, .. } => *data = Some(data_add),
			_ => panic!("Cannot set data on variant without data field."),
		}
	}
}

impl std::fmt::Display for Frame {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Header { salt, iv } => write!(
				f,
				"Header Frame (salt: {:02X?} (length: {}), iv: {:02X?} (length: {}))",
				salt,
				salt.len(),
				iv,
				iv.len()
			),
			Self::Sticker { data_length, .. } => write!(f, "Sticker (size: {})", data_length),
			Self::Attachment { data_length, .. } => write!(f, "Attachment (size: {})", data_length),
			Self::Avatar { data_length, .. } => write!(f, "Avatar (size: {})", data_length),
			Self::Preference { .. } => write!(f, "Preference"),
			Self::Statement { .. } => write!(f, "Statement"),
			Self::Version { version } => write!(f, "Version ({})", version),
			Self::End => write!(f, "End"),
			Self::KeyValue { .. } => write!(f, "KeyValue"),
		}
	}
}

impl std::convert::TryFrom<Vec<u8>> for Frame {
	type Error = anyhow::Error;

	fn try_from(data: Vec<u8>) -> Result<Self, Self::Error> {
		let mut frame = protobuf::Message::parse_from_bytes(&data)
			.with_context(|| format!("Could not parse frame from {:02X?}", &data))?;
		Ok(Self::new(&mut frame))
	}
}
