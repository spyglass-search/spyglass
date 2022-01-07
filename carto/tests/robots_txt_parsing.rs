use carto::robots::parse;

#[test]
fn test_parse() {
    let robots_txt = include_str!("./robots.txt");
    let matches = parse(robots_txt);

    assert_eq!(matches.allow.len(), 0);
    assert!(matches.disallow.len() > 0);
}
