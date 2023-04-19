pub mod filter;
pub mod get;
pub mod log;
pub mod sort;

use roaring::RoaringBitmap;

use crate::{
    fts::{lang::LanguageDetector, Language},
    write::BitmapFamily,
    BitmapKey, Serialize, BM_DOCUMENT_IDS,
};

#[derive(Debug, Clone, Copy)]
pub enum Operator {
    LowerThan,
    LowerEqualThan,
    GreaterThan,
    GreaterEqualThan,
    Equal,
}

#[derive(Debug)]
pub enum Filter {
    MatchValue {
        field: u8,
        op: Operator,
        value: Vec<u8>,
    },
    HasText {
        field: u8,
        text: String,
        op: TextMatch,
    },
    InBitmap {
        family: u8,
        field: u8,
        key: Vec<u8>,
    },
    DocumentSet(RoaringBitmap),
    And,
    Or,
    Not,
    End,
}

#[derive(Debug)]
pub enum TextMatch {
    Exact(Language),
    Stemmed(Language),
    Tokenized,
}

#[derive(Debug)]
pub enum Comparator {
    Field { field: u8, ascending: bool },
    DocumentSet { set: RoaringBitmap, ascending: bool },
}

#[derive(Debug)]
pub struct ResultSet {
    account_id: u32,
    collection: u8,
    pub results: RoaringBitmap,
}

pub struct SortedResultSet {
    pub position: i32,
    pub ids: Vec<u64>,
    pub found_anchor: bool,
}

impl Filter {
    pub fn cond(field: impl Into<u8>, op: Operator, value: impl Serialize) -> Self {
        Filter::MatchValue {
            field: field.into(),
            op,
            value: value.serialize(),
        }
    }

    pub fn eq(field: impl Into<u8>, value: impl Serialize) -> Self {
        Filter::MatchValue {
            field: field.into(),
            op: Operator::Equal,
            value: value.serialize(),
        }
    }

    pub fn lt(field: impl Into<u8>, value: impl Serialize) -> Self {
        Filter::MatchValue {
            field: field.into(),
            op: Operator::LowerThan,
            value: value.serialize(),
        }
    }

    pub fn le(field: impl Into<u8>, value: impl Serialize) -> Self {
        Filter::MatchValue {
            field: field.into(),
            op: Operator::LowerEqualThan,
            value: value.serialize(),
        }
    }

    pub fn gt(field: impl Into<u8>, value: impl Serialize) -> Self {
        Filter::MatchValue {
            field: field.into(),
            op: Operator::GreaterThan,
            value: value.serialize(),
        }
    }

    pub fn ge(field: impl Into<u8>, value: impl Serialize) -> Self {
        Filter::MatchValue {
            field: field.into(),
            op: Operator::GreaterEqualThan,
            value: value.serialize(),
        }
    }

    pub fn has_text(field: impl Into<u8>, text: impl Into<String>, mut language: Language) -> Self {
        let mut text = text.into();
        let op = if !matches!(language, Language::None) {
            let match_phrase = (text.starts_with('"') && text.ends_with('"'))
                || (text.starts_with('\'') && text.ends_with('\''));

            if !match_phrase && language == Language::Unknown {
                language = if let Some((l, t)) = text
                    .split_once(':')
                    .and_then(|(l, t)| (Language::from_iso_639(l)?, t.to_string()).into())
                {
                    text = t;
                    l
                } else {
                    LanguageDetector::detect_single(&text)
                        .and_then(|(l, c)| if c > 0.3 { Some(l) } else { None })
                        .unwrap_or(Language::Unknown)
                };
            }

            if match_phrase {
                TextMatch::Exact(language)
            } else {
                TextMatch::Stemmed(language)
            }
        } else {
            TextMatch::Tokenized
        };

        Filter::HasText {
            field: field.into(),
            text,
            op,
        }
    }

    pub fn has_english_text(field: impl Into<u8>, text: impl Into<String>) -> Self {
        Self::has_text(field, text, Language::English)
    }

    pub fn is_in_bitmap(field: impl Into<u8>, value: impl BitmapFamily + Serialize) -> Self {
        Self::InBitmap {
            family: value.family(),
            field: field.into(),
            key: value.serialize(),
        }
    }

    pub fn is_in_set(set: RoaringBitmap) -> Self {
        Filter::DocumentSet(set)
    }
}

impl Comparator {
    pub fn field(field: impl Into<u8>, ascending: bool) -> Self {
        Self::Field {
            field: field.into(),
            ascending,
        }
    }

    pub fn set(set: RoaringBitmap, ascending: bool) -> Self {
        Self::DocumentSet { set, ascending }
    }

    pub fn ascending(field: impl Into<u8>) -> Self {
        Self::Field {
            field: field.into(),
            ascending: true,
        }
    }

    pub fn descending(field: impl Into<u8>) -> Self {
        Self::Field {
            field: field.into(),
            ascending: false,
        }
    }
}

impl BitmapKey<&'static [u8]> {
    pub fn document_ids(account_id: u32, collection: impl Into<u8>) -> Self {
        BitmapKey {
            account_id,
            collection: collection.into(),
            family: BM_DOCUMENT_IDS,
            field: u8::MAX,
            key: b"",
            block_num: 0,
        }
    }
}