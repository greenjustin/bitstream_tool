use std::collections::VecDeque;

pub struct SyntaxField {
    pub name: String,
    pub val: i32,
}

pub struct SyntaxNode {
    pub name: String,
    pub children: VecDeque<SyntaxElement>,
}

pub struct SyntaxPayload {
    pub name: String,
    pub data: Vec<u8>,
}

pub enum SyntaxElement {
    Field(SyntaxField),
    Node(SyntaxNode),
    Payload(SyntaxPayload),
}

impl ToString for SyntaxElement {
    fn to_string(&self) -> String {
        match self {
            SyntaxElement::Field(field) => format!("{}: {}\n", field.name, field.val.to_string()),
            SyntaxElement::Node(node) => {
                let mut ret: String = format!("{} {{\n", node.name);
                for element in &node.children {
                    for line in element.to_string().split('\n') {
                        if line.trim().is_empty() {
                            continue;
                        }
                        ret = format!("{}\t{}\n", ret, line);
                    }
                }
                format!("{}}}\n", ret)
            },
            SyntaxElement::Payload(payload) => {
                format!("{}: \"{}\"\n", payload.name, payload.data.iter()
                    .map(|x| format!("{:02X}", x).to_string())
                    .collect::<Vec<String>>()
                    .join(" "))
            },
        }
    }
}

pub fn syntax_elements_from_string(mut rows: &mut VecDeque<String>) -> VecDeque<SyntaxElement> {
    let mut ret: VecDeque<SyntaxElement> = VecDeque::new();
    loop {
        let Some(mut row) = rows.pop_front() else {
            break;
        };
        row = row.trim().to_string();
        if row == "}" {
            break;
        } else if row.ends_with(" {") {
            let name = row.replace(" {", "");
            let children = syntax_elements_from_string(&mut rows);
            ret.push_back(SyntaxElement::Node(SyntaxNode { name: name.to_string(), children: children }));
        } else if row.contains(":") {
            let (name, val) = row.split_at(row.find(":").unwrap());
            if val.starts_with(": \"") && val.ends_with("\"") {
                let mut data: Vec<u8> = vec![];
                for byte in val.strip_prefix(": \"").unwrap().strip_suffix("\"").unwrap().split(' ') {
                    data.push(u8::from_str_radix(byte, 16).unwrap());
                }
                ret.push_back(SyntaxElement::Payload(SyntaxPayload { name: name.to_string(), data: data } ));
            } else {
                let converted_val = i32::from_str_radix(val.strip_prefix(": ").unwrap(), 10).unwrap();
                ret.push_back(SyntaxElement::Field(SyntaxField { name: name.to_string(), val: converted_val } ));
            }
        }
    }

    ret
}

pub enum FieldType {
    Boolean,
    UnsignedInt,
    SignedInt,
    UnsignedExpGolomb,
    SignedExpGolomb,
}

pub trait BitstreamProcessor {
    fn field(&mut self, node: &mut SyntaxNode, name: &str, field_type: FieldType, n: u8) -> i32;
    fn subnode<A>(&mut self, node: &mut SyntaxNode, name: &str, cb: A) -> ()
        where A: FnMut(&mut SyntaxNode, &mut Self) -> ();
    fn payload(&mut self, node: &mut SyntaxNode, name: &str) -> ();
    fn more_data(&mut self, node: &mut SyntaxNode) -> bool;
}

pub struct BitstreamReader<'a> {
    buffer: &'a [u8],
    bit_index: usize,
}

impl BitstreamReader<'_> {
    fn peek_bit(&self) -> Option<i32> {
        if self.bit_index / 8 >= self.buffer.len() {
            None
        } else {
            let byte = self.buffer[self.bit_index / 8];
            Some(i32::from(((byte << (self.bit_index % 8)) & 0b10000000) >> 7))
        }
    }

    fn read_bit(&mut self) -> Option<i32> {
        let ret = self.peek_bit()?;
        self.bit_index += 1;

        Some(ret)
    }
    fn read_bits(&mut self, n: u8, init_val: i32) -> Option<i32> {
        if n > 64 {
            panic!("Cannot read more than 64 bits from bitstream");
        }

        let mut ret: i32 = init_val;
        for _i in 0..n {
            ret = (ret << 1) | i32::from(self.read_bit()?);
        }

        Some(ret)
    }

    pub fn read(&mut self, field_type: FieldType, n: u8) -> Option<i32> {
        match field_type {
            FieldType::Boolean => self.read_bit(),
            FieldType::UnsignedInt => self.read_bits(n, 0),
            FieldType::SignedInt => {
                let sign = self.read_bit()?;
                self.read_bits(n-1, if sign == 1 { -1 } else { 0 })
            },
            FieldType::UnsignedExpGolomb => {
                let mut len = 0;
                let mut bit = self.read_bit()?;
                while bit == 0 {
                    len += 1;
                    bit = self.read_bit()?;
                }
                Some(((1 << len) | self.read(FieldType::UnsignedInt, len)?) - 1)
            },
            FieldType::SignedExpGolomb => {
                let val = self.read(FieldType::UnsignedExpGolomb, 0)?;
                if val % 2 == 1 {
                    return Some(val / 2 + 1)
                } else {
                    return Some(val / -2)
                }
            },
        }
    }

    pub fn new(buffer: &[u8]) -> BitstreamReader {
        BitstreamReader { buffer: buffer, bit_index: 0 }
    }
}

impl BitstreamProcessor for BitstreamReader<'_> {
    fn field(&mut self, node: &mut SyntaxNode, name: &str, field_type: FieldType, n: u8) -> i32 {
        let ret = self.read(field_type, n).expect(&format!("Bitstream ended unexpectedly while parsing {}", name));
        node.children.push_back(SyntaxElement::Field(SyntaxField {name: name.to_string(), val: ret}));
        ret
    }

    fn subnode<A>(&mut self, node: &mut SyntaxNode, name: &str, mut cb: A) -> ()
        where A: FnMut(&mut SyntaxNode, &mut Self) -> () {
        let mut subnode = SyntaxNode {name: name.to_string(), children: VecDeque::new()};
        cb(&mut subnode, self);
        node.children.push_back(SyntaxElement::Node(subnode));
    }

    fn payload(&mut self, node: &mut SyntaxNode, name: &str) -> () {
        let mut payload: Vec<u8> = vec![];
        if self.bit_index % 8 != 0 {
            payload.push(self.read(FieldType::UnsignedInt, (8 - (self.bit_index % 8)).try_into().unwrap())
                .unwrap().try_into().unwrap());
        }
        for i in (self.bit_index/8)..self.buffer.len() {
            payload.push(self.buffer[i]);
        }
        node.children.push_back(SyntaxElement::Payload(SyntaxPayload {name: name.to_string(), data: payload}));
    }

    fn more_data(&mut self, node: &mut SyntaxNode) -> bool {
        if self.bit_index/8 == self.buffer.len()-1 {
            (self.buffer[self.buffer.len()-1] & ((1 << (8 - self.bit_index % 8)) - 1)).count_ones() != 1
        } else if self.bit_index/8 < self.buffer.len()-1 {
            true
        } else {
            false
        }
    }
}

pub struct BitstreamWriter {
    pub buffer: Vec<u8>,
    bit_index: usize,
}

impl BitstreamWriter {
    fn write_bit(&mut self, bit: bool) -> () {
        let byte_index = self.bit_index / 8;
        while byte_index >= self.buffer.len() {
            self.buffer.push(0);
        }
        self.buffer[byte_index] |= u8::from(bit) << (7 - (self.bit_index % 8));
        self.bit_index += 1;
    }

    pub fn write(&mut self, field_type: FieldType, n: u8, val: i32) -> () {
        if n > 64 {
            panic!("Cannot write bitfield of size greater than 64");
        }
        match field_type {
            FieldType::Boolean => self.write_bit(val != 0),
            FieldType::UnsignedExpGolomb => {
                let num_len = 32 - (val+1).leading_zeros();
                self.write(FieldType::UnsignedInt, (num_len-1).try_into().unwrap(), 0);
                self.write(FieldType::UnsignedInt, (num_len).try_into().unwrap(), val+1);
            },
            FieldType::SignedExpGolomb => {
                if val > 0 {
                    self.write(FieldType::UnsignedExpGolomb, 0, 2 * val - 1);
                } else {
                    self.write(FieldType::UnsignedExpGolomb, 0, -2 * val);
                }
            },
            _ => {
                // Signed and unsigned are handled the same
                for i in 0..n {
                    self.write_bit(((val >> (n-1-i)) & 0x1) != 0);
                }
            },
        }
    }

    pub fn new() -> BitstreamWriter {
        BitstreamWriter { buffer: vec![], bit_index: 0 }
    }
}

impl BitstreamProcessor for BitstreamWriter {
    fn field(&mut self, node: &mut SyntaxNode, name: &str, field_type: FieldType, n: u8) -> i32 {
        let SyntaxElement::Field(child) = node.children.pop_front().expect(&format!("Expected {} but got nothing!", name)) else {
            panic!("Invalid syntax element at {name}");
        };
        assert_eq!(child.name, name, "Expected {}, got {}", name, child.name);
        self.write(field_type, n, child.val);
        child.val
    }

    fn subnode<A>(&mut self, node: &mut SyntaxNode, name: &str, mut cb: A) -> ()
        where A: FnMut(&mut SyntaxNode, &mut Self) -> () {
        let SyntaxElement::Node(mut subnode) = node.children.pop_front().expect(&format!("Expected {} but got nothing!", name)) else {
            panic!("Invalid syntax element at {name}");
        };
        assert_eq!(subnode.name, name, "Expected {}, got {}", name, subnode.name);
        cb(&mut subnode, self);
    }

    fn payload(&mut self, node: &mut SyntaxNode, name: &str) -> () {
        let SyntaxElement::Payload(child) = node.children.pop_front().expect(&format!("Expected {} but got nothing!", name)) else {
            panic!("Invalid syntax element at {name}");
        };
        assert_eq!(child.name, name, "Expected {}, got {}", name, child.name);
        let start_idx = if self.bit_index % 8 != 0 && child.data.len() > 0 {
            self.write(FieldType::UnsignedInt,
                       (8 - (self.bit_index % 8)).try_into().unwrap(),
                       i32::from(child.data[0] & ((1 << (8 - (self.bit_index % 8))) - 1)));
            1
        } else {
            0
        };
        for i in start_idx..child.data.len() {
            self.write(FieldType::UnsignedInt, 8, i32::from(child.data[i]));
        }
    }

    fn more_data(&mut self, node: &mut SyntaxNode) -> bool {
        match node.children.len() {
            0 => false,
            1 => match node.children[0] {
                SyntaxElement::Payload(_) => false,
                _ => true,
            },
            _ => true,
        }
    }
}
