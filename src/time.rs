use chrono::Local;

pub fn get_current_timestamp() -> i64 {
    Local::now().timestamp()
}

#[cfg(test)]
mod test {
    use super::*;
    //use chrono::DateTime;
    use chrono::prelude::*;
    use utils::dbgt;

    #[test]
    fn test_time() {
        println!("Now time is {:?}", std::time::SystemTime::now());
        let local: DateTime<Local> = Local::now();
        // println!("{:?}", local.format("%Y-%m-%d %H:%M:%S").to_string());
        // println!("{:?}", local.format("%Y-%m-%d %H:%M:%S%.3f").to_string());
        dbgt!(&local.timestamp());
    }
}
