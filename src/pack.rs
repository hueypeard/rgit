use flate2::read::ZlibDecoder;

use std::fs::File;
use std::io::{Read,ReadExt,Seek,Cursor};

static MAGIC_HEADER: u32 = 1346454347; // "PACK"

pub struct PackFile {
    version: u32,
    num_objects: u32,
    objects: Vec<PackfileObject>
}

pub struct PackfileObject {
    obj_type: PackObjectType,
    size: uint,
    content: Vec<u8>
}

pub enum PackObjectType {
    Commit,
    Tree,
    Blob,
    Tag,
    OfsDelta(u8),
    RefDelta([u8; 20]),
}

impl PackFile {
    pub fn from_file(mut file: File) -> Self {
        // Read header bytes in big-endian format<LeftMouse>
        let magic = read_be_u32(&mut file);
        let version = read_be_u32(&mut file);
        let num_objects = read_be_u32(&mut file);

        if magic == MAGIC_HEADER {
            let objects = read_packfile_objects(&mut file, num_objects);
            PackFile {
                version: version,
                num_objects: num_objects,
                objects: objects
            }
        } else {
          unreachable!("Packfile failed to parse");
        }
    }
}

fn read_packfile_objects(file: &mut File, num_objects: u32) -> Vec<PackfileObject> {
    let mut objects = Vec::new();

    let mut contents = Vec::new();
    file.read_to_end(&mut contents);
    let mut cursor = Cursor::new(contents);
    let mut total_in = 0u64;

    for i in 0..num_objects {
      let mut c = read_byte(&mut cursor);
      let type_id = (c >> 4) & 7;

      let mut size: usize = (c & 15) as usize;
      let mut shift: usize = 4;

      // Parse the variable length size header for the object.
      // Read the MSB and check if we need to continue
      // consuming bytes to get the object size
      while c & 0x80 > 0 {
          c = read_byte(&mut cursor);
          size += ((c & 0x7f) as usize) << shift;
          shift += 7;
      }

      let obj_type = read_object_type(&mut cursor, type_id).expect(
          "Error parsing object type in packfile"
          );

      let content = read_object_content(&mut cursor, size);
      let obj = PackfileObject {
          obj_type: obj_type,
          size: size,
          content: content
      };
      objects.push(obj);
    }
    objects
}

// Reads exactly size bytes of zlib inflated data from the filestream.
fn read_object_content(in_data: &mut Cursor<Vec<u8>>, size: usize) -> Vec<u8> {
    use std::io::Seek;
    use std::io::SeekFrom;

    let current = in_data.position();

    let (content, new_pos) = {
      let mut z = ZlibDecoder::new(in_data.by_ref());
      let mut buf = Vec::with_capacity(size);
      match z.read(&mut buf[..]) {
          Ok(read_size) if read_size == size => (buf, z.total_in() + current),
          _ => panic!("Wat")
      }
    };
    in_data.seek(SeekFrom::Start(new_pos));
    content
}

fn read_object_type<R>(r: &mut R, id: u8) -> Option<PackObjectType> where R: Read {
    match id {
        1 => Some(PackObjectType::Commit),
        2 => Some(PackObjectType::Tree),
        3 => Some(PackObjectType::Blob),
        4 => Some(PackObjectType::Tag),
        6 => {
            Some(PackObjectType::OfsDelta(read_offset(r)))
        },
        7 => {
            let mut base: [u8; 20] = [0; 20];
            for i in range(0, 20) {
                base[i] = read_byte(r);
            }
            Some(PackObjectType::RefDelta(base))
        }
        _ => None
    }
}

// Offset encoding.
// n bytes with MSB set in all but the last one.
// The offset is then the number constructed
// by concatenating the lower 7 bits of each byte, and
// for n >= 2 adding 2^7 + 2^14 + ... + 2^(7*(n-1))
// to the result.
fn read_offset<R>(r: &mut R) -> u8 where R: Read {
    let mut shift = 0;
    let mut c;
    let mut offset = 0;
    loop {
        c = read_byte(r);
        offset += (c & 0x7f) << shift;
        shift += 7;
    }
    offset
}

fn read_byte<R>(r: &mut R) -> u8 where R: Read {
  let mut buf = [0];
  match r.read(&mut buf) {
    Ok(s) if s == 1 => buf[0],
    _ => panic!("error read_byte")
  }
}

fn read_be_u32<R>(r: &mut R) -> u32 where R: Read {
  let mut buf = [0; 4];
  match r.read(&mut buf) {
    Ok(s) if s == 4 => {
        let mut result = 0u32;

        // This is because I already know my system is be
        for i in buf.iter() {
          result = result << 8;
          result += *i as u32;
        }
        result
    },
    _ => panic!("error read_be_32")
  }
}

