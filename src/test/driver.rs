use super::*;

fn dummy_device() -> DevProxy {
    dummy_dev_port(8000)
}

fn dummy_dev_port(port: u16) -> DevProxy {
    DevProxy {
        addr: Addr::Dns(String::from("localhost"), port),
    }
}

#[test]
fn add_remove() {
    let mut drv = Driver::new().unwrap();
    let dev = dummy_device();
    let dh = drv.attach(dev).unwrap();
    dh.detach().unwrap();
}

#[test]
fn add_remove_many() {
    let mut drv = Driver::new().unwrap();
    let devs = (0..16).map(|i| dummy_dev_port(8000 + i));
    let mut dhs = Vec::new();
    for dev in devs {
        dhs.push(drv.attach(dev).unwrap());
    }
    for (i, dh) in dhs.drain(..).enumerate() {
        let dev = dh.detach().unwrap();
        if let Addr::Dns(_, p) = dev.addr {
            assert_eq!(p as usize, 8000 + i);
        } else {
            panic!();
        }
    }
    assert_eq!(dhs.len(), 0);
}
