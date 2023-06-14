use super::*;
use ruc::*;

#[test]
fn test_insert() {
    let mut hdr = MapxOrdRawKey::new();
    let max = 100;
    (0..max)
        .map(|i: usize| (i.to_be_bytes(), (max + i).to_be_bytes()))
        .for_each(|(key, value)| {
            let key = key.to_vec();
            let value = value.to_vec();
            assert!(hdr.get(&key).is_none());
            hdr.entry(&key[..]).or_insert(value.clone());
            hdr.set_value(&key[..], &value);
            assert!(hdr.insert(&key[..], &value).is_some());
            assert!(hdr.contains_key(&key[..]));
            assert_eq!(pnk!(hdr.get(&key[..])), value);
            assert_eq!(pnk!(hdr.remove(&key[..])), value);
            assert!(hdr.get(&key[..]).is_none());
            assert!(hdr.insert(&key[..], &value).is_none());
        });
    hdr.clear();
    (0..max).map(|i: usize| i.to_be_bytes()).for_each(|key| {
        assert!(hdr.get(&key).is_none());
    });
    assert!(hdr.is_empty());
}
#[test]
fn test_len() {
    let mut hdr = MapxOrdRawKey::new();
    let max = 100;
    (0..max)
        .map(|i: usize| (i.to_be_bytes(), max + i))
        .for_each(|(key, value)| {
            assert!(hdr.insert(&key[..], &value).is_none());
        });
    assert_eq!(100, hdr.len());

    for key in 0..max {
        assert!(hdr.remove(&key.to_be_bytes()).is_some());
    }
    assert_eq!(0, hdr.len());
}

#[test]
fn test_valueende() {
    let cnt = 100;
    let dehdr = {
        let mut hdr = MapxOrdRawKey::new();
        (0..cnt)
            .map(|i: usize| (i.to_be_bytes(), i))
            .for_each(|(key, value)| {
                assert!(hdr.insert(&key[..], &value).is_none());
            });
        <MapxOrdRawKey<usize> as ValueEnDe>::encode(&hdr)
    };
    let mut reloaded = pnk!(<MapxOrdRawKey<usize> as ValueEnDe>::decode(&dehdr));
    assert_eq!(cnt, reloaded.len());
    (0..cnt).map(|i: usize| i).for_each(|i| {
        assert_eq!(i, reloaded.get(&i.to_be_bytes()).unwrap());
    });
}

#[test]
fn test_iter() {
    let mut hdr = MapxOrdRawKey::new();
    let max = 100;
    (0..max)
        .map(|i: usize| (i.to_be_bytes(), i))
        .for_each(|(key, value)| {
            assert!(hdr.insert(&key[..], &value).is_none());
        });
    for (key, _) in hdr.iter().collect::<Vec<_>>().into_iter() {
        hdr.unset_value(&key);
    }
    assert_eq!(0, hdr.len());
}

#[test]
fn test_first_last() {
    let mut hdr = MapxOrdRawKey::new();
    let max = 100;
    (0..max)
        .map(|i: usize| (i.to_be_bytes(), i))
        .for_each(|(key, value)| {
            assert!(hdr.insert(&key[..], &value).is_none());
        });
    let (_, value) = pnk!(hdr.first());
    assert_eq!(0, value);

    let (_, value) = pnk!(hdr.last());
    assert_eq!(max - 1, value);
}

#[test]
fn test_values() {
    let mut hdr = MapxOrdRawKey::new();
    let max = 100;
    (0..max)
        .map(|i: usize| (i.to_be_bytes(), i))
        .for_each(|(key, value)| {
            assert!(hdr.insert(&key[..], &value).is_none());
        });
    let mut i = 0;
    for (_, it) in hdr.iter() {
        assert_eq!(i, it);
        i = i + 1;
    }
}

#[test]
fn test_values_first_last() {
    let mut hdr = MapxOrdRawKey::new();
    let max = 100;
    (0..max)
        .map(|i: usize| (i.to_be_bytes(), i))
        .for_each(|(key, value)| {
            assert!(hdr.insert(&key[..], &value).is_none());
        });
    let (_, value) = pnk!(hdr.iter().next());
    assert_eq!(0, value);

    let (_, value) = pnk!(hdr.iter().next_back());
    assert_eq!(max - 1, value);
}
