use lan_link_protocol::frame::CrontabActionType;

pub fn cmd_crontab(action: &CrontabActionType) -> (Vec<u8>, Option<i32>) {
    match action {
        CrontabActionType::List => {
            let user = std::env::var("USER").unwrap_or_else(|_| "root".to_string());
            match std::fs::read_to_string(&format!("/var/spool/cron/crontabs/{}", user)) {
                Ok(c) => (c.into_bytes(), Some(0)),
                Err(_) => match std::fs::read_to_string("/etc/crontab") {
                    Ok(c) => {
                        let mut out = "# (from /etc/crontab, user crontab not found)\n".to_string();
                        out.push_str(&c);
                        (out.into_bytes(), Some(0))
                    }
                    Err(_) => (b"no crontab for this user\n".to_vec(), Some(1)),
                },
            }
        }
        CrontabActionType::Edit => (b"Use 'iexec crontab -e' for editing\n".to_vec(), Some(0)),
        CrontabActionType::Remove => {
            let user = std::env::var("USER").unwrap_or_else(|_| "root".to_string());
            match std::fs::remove_file(&format!("/var/spool/cron/crontabs/{}", user)) {
                Ok(_) => (b"crontab removed\n".to_vec(), Some(0)),
                Err(_) => (b"no crontab to remove\n".to_vec(), Some(1)),
            }
        }
    }
}
