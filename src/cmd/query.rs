use std::io::{self, Write};

use anyhow::{bail, Result};
use skim::prelude::*;

use crate::cmd::{Query, Run};
use crate::config;
use crate::db::{Database, DirItem, Epoch, Stream};
use crate::error::BrokenPipeHandler;
use crate::util::current_time;

impl Run for Query {
    fn run(&self) -> Result<()> {
        let mut db = crate::db::Database::open()?;
        self.query(&mut db).and(db.save())
    }
}
// // Search mode
// "--exact",
// // Search result
// "--no-sort",
// // Interface
// "--bind=ctrl-z:ignore,btab:up,tab:down",
// "--cycle",
// "--keep-right",
// // Layout
// "--border=sharp", // rounded edges don't display correctly on some terminals
// "--height=45%",
// "--info=inline",
// "--layout=reverse",
// // Display
// "--tabstop=1",
// // Scripting
// "--exit-0",
// "--select-1",

impl Query {
    fn query(&self, db: &mut Database) -> Result<()> {
        let now = current_time()?;
        let mut stream = self.get_stream(db, now);

        if self.interactive {
            let (tx_item, rx_item): (SkimItemSender, SkimItemReceiver) = unbounded();
            let options = SkimOptionsBuilder::default()
                .height(Some("45%"))
                .multi(false)
                // .preview(Some("command -p ls -Cp --color=always --group-directories-first {}"))
                .preview(Some("command -p ls -Cp --color=always {}"))
                // .no_clear(true)
                // .no_clear_start(true)
                .build()
                .unwrap();

            while let Some(dir) = stream.next() {
                let ddd = DirItem::from(dir);
                let _ = tx_item.send(Arc::new(ddd));
            }
            drop(tx_item);

            let selected_items = Skim::run_with(&options, Some(rx_item))
                .map(|out| out.selected_items)
                .unwrap_or_default()
                .iter()
                .map(|selected_item| {
                    (**selected_item).as_any().downcast_ref::<DirItem>().unwrap().to_owned()
                })
                .collect::<Vec<DirItem>>();

            if let Some(selection) = selected_items.get(0) {
                if self.score {
                    print!("{} {}", selection.rank, selection.path);
                } else {
                    print!("{}", selection.path);
                }
            }
        } else if self.list {
            let handle = &mut io::stdout().lock();
            while let Some(dir) = stream.next() {
                let dir = if self.score { dir.display().with_score(now) } else { dir.display() };
                writeln!(handle, "{dir}").pipe_exit("stdout")?;
            }
        } else {
            let handle = &mut io::stdout();
            let Some(dir) = stream.next() else {
                bail!(if stream.did_exclude() {
                    "you are already in the only match"
                } else {
                    "no match found"
                });
            };
            let dir = if self.score { dir.display().with_score(now) } else { dir.display() };
            writeln!(handle, "{dir}").pipe_exit("stdout")?;
        }

        Ok(())
    }

    fn get_stream<'a>(&self, db: &'a mut Database, now: Epoch) -> Stream<'a> {
        let mut stream = db.stream(now).with_keywords(&self.keywords);
        if !self.all {
            let resolve_symlinks = config::resolve_symlinks();
            stream = stream.with_exists(resolve_symlinks);
        }
        if let Some(path) = &self.exclude {
            stream = stream.with_exclude(path);
        }
        stream
    }
}
