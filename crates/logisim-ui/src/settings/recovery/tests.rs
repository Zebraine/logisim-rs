#[test]
fn utc_dates_cover_epoch_leap_day_and_gregorian_cycle() {
    for (stamp, date) in [
        (0, "1970-01-01 00:00 UTC"),
        (951827696, "2000-02-29 12:34 UTC"),
        (253402300799, "9999-12-31 23:59 UTC"),
    ] {
        assert_eq!(super::utc_time(stamp), date);
    }
}
