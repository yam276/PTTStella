//! Cell targets are derived from the same acknowledged filtered Paint used by
//! selection and export. No terminal text or hidden source is reparsed here.
use super::*;
#[derive(Clone)]
pub struct AccountContext {
    pub identity: Identity,
    pub stamp: Stamp,
    pub cell: Option<(usize, usize)>,
    pub word: Option<pttstella::filters::AccountWord>,
    pub reply: Option<usize>,
    pub author: bool,
    pub focused: bool,
}
impl Output {
    pub fn account_context(
        &self,
        ui: &TerminalWindow,
        point: (f32, f32),
    ) -> Option<AccountContext> {
        self.current(ui)?;
        let article = self.article.as_ref()?;
        let geometry = self.geometry(ui)?;
        let view = self.connection.updates.borrow();
        article
            .with_output(&view, |paint| {
                if !paint.ptt_features {
                    return None;
                }
                let cell = geometry.cell(point, &paint.frame).map(|p| (p.row, p.col));
                // Both core producers emit words in row order. Restrict the
                // pointer lookup to one row, even for a long cached article.
                let word = cell.and_then(|(row, col)| {
                    let start = paint.account_words.partition_point(|w| w.row < row);
                    paint.account_words[start..]
                        .iter()
                        .enumerate()
                        .take_while(|(_, w)| w.row == row)
                        .find(|(_, w)| w.start <= col && col < w.end)
                        .map(|(offset, w)| (start + offset, w))
                });
                Some(AccountContext {
                    identity: Identity::of(paint),
                    stamp: view.stamp(),
                    cell,
                    author: word.is_some_and(|(i, _)| paint.reply_author_words.contains(&i)),
                    word: word.map(|(_, w)| w.clone()),
                    reply: cell
                        .map(|p| p.0)
                        .filter(|row| paint.reply_rows.contains(row)),
                    focused: paint.reply_focus.is_some(),
                })
            })
            .flatten()
    }
    pub fn focus_account(&self, target: &AccountContext, clear: bool) -> bool {
        self.article.as_ref().is_some_and(|a| {
            a.focus_reply(
                target.identity,
                target.stamp,
                if clear { None } else { target.reply },
            )
        })
    }
    pub fn filter_status(&self, ui: &TerminalWindow) {
        let value = self.article.as_ref().and_then(|a| {
            a.with_output(&self.connection.updates.borrow(), |p| {
                (p.masked_pushes, p.revealed)
            })
        });
        ui.set_filter_ready(value.is_some());
        let (count, revealed) = value.unwrap_or_else(|| {
            (
                ui.get_filter_count().max(0) as usize,
                ui.get_filter_revealed(),
            )
        });
        ui.set_filter_revealed(revealed);
        ui.set_filter_count(i32::try_from(count).unwrap_or(i32::MAX));
        ui.set_filter_label(
            if revealed {
                msg!("filters-mask")
            } else {
                msg!("filters-reveal")
            }
            .into(),
        );
        ui.set_filter_status(
            if count == 0 {
                String::new()
            } else if revealed {
                msg!("filters-count-revealed", count = count)
            } else {
                msg!("filters-count-hidden", count = count)
            }
            .into(),
        );
    }
}
