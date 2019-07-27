pub fn is_begin(ch: &u8) -> bool {
    *ch == b'^'
}

    pub fn is_repeat(ch: &u8) -> bool{
        *ch == b'?' || *ch == b'+' || *ch == b'*'
    }

    pub fn is_end(ch: &u8) -> bool{
         *ch == b'$'
    }

    pub fn is_slash(ch: &u8) -> bool{
         *ch == b'\\'
    }

    pub fn is_set_begin(ch: &u8) -> bool{
         *ch == b'['
    }

    pub fn is_set_end(ch: &u8) -> bool{
         *ch == b']'
    }

    pub fn is_group_begin(ch: &u8) -> bool{
         *ch == b'('
    }

    pub fn is_group_end(ch: &u8) -> bool{
         *ch == b')'
    }

    pub fn is_select(ch: &u8) -> bool{
         *ch == b'|'
    }

    pub fn is_repeat_begin(ch: &u8) -> bool{
         *ch == b'{'
    }

    pub fn is_repeat_end(ch: &u8) -> bool{
         *ch == b'}'
    }

    pub fn is_needend(ch: &u8) -> bool {
         is_group_end(ch) || is_repeat_end(ch)
    }

    pub fn is_digit(ch: &u8) -> bool {
         b'0' <= *ch && *ch <= b'9'
    }

    pub fn trans_digit(ch: &u8) -> u8 {
         (*ch - b'0')
    }

    pub fn is_dash(ch: &u8) -> bool {
         *ch == b'-'
    }

    pub fn is_any(ch: &u8) -> bool {
         *ch == b'.'
    }

    pub fn is_subexp_mark_ch(ch: &u8) -> &u8 {
         if *ch == b':' || *ch == b'=' || *ch == b'!' || *ch == b'>' {
             ch
         } else {
             &(0 as u8)         
         }
    }

    pub fn is_subexp_mark(ch: &[u8]) -> &u8 {
        let mut piter = ch.iter().peekable();
        if let Some(ch) = piter.next() {
            if *ch == b'?' {
                if let Some(p) = piter.peek() {
                    is_subexp_mark_ch(p)
                } else {
                    &(0 as u8)
                }
            } else {
                &(0 as u8)
            }
        } else {
            &(0 as u8)
        }
    }

    pub fn trans_slash(ch: &u8) -> &u8 {
        match ch {
            b'n' => &b'\n',
            b'r' => &b'\r',
            b't' => &b'\t',
            _ => ch
        }
    }