use super::*;

#[test]
fn duplicates_extend_colliding_suffixes_and_unique_names_have_none() {
    let first = LogbookTag::new("00000000000000000000000001".parse().unwrap(), "Main".into());
    let second = LogbookTag::new("00000000000000000100000001".parse().unwrap(), "Main".into());
    let unique = LogbookTag::new(TagId::new(), "Portable".into());
    let captions = labels(&[&first, &second, &unique]);
    assert_eq!(captions.get(&first.id).unwrap(), "Main · …000000001");
    assert_eq!(captions.get(&second.id).unwrap(), "Main · …100000001");
    assert_eq!(captions.get(&unique.id).unwrap(), "Portable");
    let mut renamed = second;
    renamed.name = "Other".into();
    assert_eq!(labels(&[&first, &renamed]).get(&first.id).unwrap(), "Main");
}

#[test]
fn ordinary_duplicate_suffixes_keep_full_identity_out_of_names() {
    let first = LogbookTag::new("00000000000000000000000001".parse().unwrap(), "Main".into());
    let second = LogbookTag::new("00000000000000000000000002".parse().unwrap(), "Main".into());
    let captions = labels(&[&second, &first]);
    assert_eq!(captions.get(&first.id).unwrap(), "Main · …00000001");
    assert_eq!(captions.get(&second.id).unwrap(), "Main · …00000002");
}
