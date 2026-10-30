use super::{parse_hex, EventColour};

const EVENT_COLOURS: [(&str, &str); 11] = [
    ("1", "#7986cb"),
    ("2", "#33b679"),
    ("3", "#8e24aa"),
    ("4", "#e67c73"),
    ("5", "#f6bf26"),
    ("6", "#f4511e"),
    ("7", "#039be5"),
    ("8", "#616161"),
    ("9", "#3f51b5"),
    ("10", "#0b8043"),
    ("11", "#d50000"),
];

const CALENDAR_COLOURS: [(&str, &str); 24] = [
    ("#ac725e", "#795548"),
    ("#d06b64", "#e67c73"),
    ("#f83a22", "#d50000"),
    ("#fa573c", "#f4511e"),
    ("#ff7537", "#ef6c00"),
    ("#ffad46", "#f09300"),
    ("#42d692", "#009688"),
    ("#16a765", "#0b8043"),
    ("#7bd148", "#7cb342"),
    ("#b3dc6c", "#c0ca33"),
    ("#fbe983", "#e4c441"),
    ("#fad165", "#f6bf26"),
    ("#92e1c0", "#33b679"),
    ("#9fe1e7", "#039be5"),
    ("#9fc6e7", "#4285f4"),
    ("#4986e7", "#3f51b5"),
    ("#9a9cff", "#7986cb"),
    ("#b99aff", "#b39ddb"),
    ("#c2c2c2", "#616161"),
    ("#cabdbf", "#a79b8e"),
    ("#cca6ac", "#ad1457"),
    ("#f691b2", "#d81b60"),
    ("#cd74e6", "#8e24aa"),
    ("#a47ae2", "#9e69af"),
];

const DEFAULT_CALENDAR_COLOUR: [u8; 3] = [0x42, 0x85, 0xf4];
const TEXT: [u8; 3] = [0x1d, 0x1d, 0x1d];

fn with_text(background: [u8; 3]) -> EventColour {
    EventColour {
        background,
        foreground: TEXT,
    }
}

pub fn event_colour(color_id: &str) -> Option<EventColour> {
    let (_, hex) = EVENT_COLOURS.iter().find(|(id, _)| *id == color_id)?;
    parse_hex(hex).map(with_text)
}

pub fn calendar_colour(background_color: Option<&str>) -> EventColour {
    let background = background_color.map(|hex| {
        CALENDAR_COLOURS
            .iter()
            .find(|(api, _)| api.eq_ignore_ascii_case(hex))
            .map_or(hex, |(_, web)| web)
    });
    with_text(
        background
            .and_then(parse_hex)
            .unwrap_or(DEFAULT_CALENDAR_COLOUR),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_colour_ids_use_the_web_palette() {
        assert_eq!(
            event_colour("6"),
            Some(EventColour {
                background: [0xf4, 0x51, 0x1e],
                foreground: TEXT,
            })
        );
        assert_eq!(event_colour("99"), None);
    }

    #[test]
    fn legacy_calendar_colours_map_to_the_web_palette() {
        assert_eq!(
            calendar_colour(Some("#9FE1E7")).background,
            [0x03, 0x9b, 0xe5]
        );
    }

    #[test]
    fn custom_calendar_colours_pass_through() {
        assert_eq!(
            calendar_colour(Some("#123456")).background,
            [0x12, 0x34, 0x56]
        );
        assert_eq!(calendar_colour(None).background, DEFAULT_CALENDAR_COLOUR);
    }
}
