use super::*;

fn dummy_device() -> DevProxy {
    DevProxy {
        addr: Addr::Dns(String::from("localhost"), 8000),
    }
}

#[test]
fn add_remove() {
    let mut drv = Driver::new().unwrap();
    let dev = dummy_device();
    let dh = drv.attach(dev).unwrap();
    dh.detach().unwrap();
}

/*
#[test]
fn add_remove() {
    let mut drv = Driver::new().unwrap();
    let dev = dummy_device();
    let devid = drv.add(dev).unwrap();
    assert!(drv.remove(devid).is_ok());
}

#[test]
fn remove_empty() {
    let mut drv = Driver::new().unwrap();
    assert!(drv.remove(0).is_err());
}

#[test]
fn remove_twice() {
    let mut drv = Driver::new().unwrap();
    let dev = dummy_device();
    let devid = drv.add(dev).unwrap();
    assert!(drv.remove(devid).is_ok());
    assert!(drv.remove(devid).is_err());
}

#[test]
fn add_remove_many() {
    let mut drv = Driver::new().unwrap();
    let devs = (0..16).map(|_| dummy_device());
    let mut ids = Vec::new();
    for dev in devs {
        ids.push(drv.add(dev).unwrap());
    }
    for id in ids {
        drv.remove(id).unwrap();
    }
}
*/