//! Page access to a 1Cv8.1CD file: the header page and fixed-size pages.
//!
//! Page 0 holds the file header:
//!
//! | offset | size | value                                        |
//! |--------|------|----------------------------------------------|
//! | 0      | 8    | signature `1CDBMSV8`                         |
//! | 8      | 4    | format version bytes, `8 3 8 0`              |
//! | 12     | 4    | number of pages in the file                  |
//! | 16     | 4    | unknown, 1 in every observed file            |
//! | 20     | 4    | page size (4096 … 65536)                     |
//!
//! Page 1 is the free-pages object, page 2 the root object (table list).

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};

use crate::error::{Error, Result, corrupt};

pub(crate) const SIGNATURE: &[u8; 8] = b"1CDBMSV8";
pub(crate) const VERSION_838: [u8; 4] = [8, 3, 8, 0];

/// What a pager reads and writes: an owned file, or a borrowed one -- a
/// write session reads through the very descriptor that holds its lock,
/// since closing any descriptor of the file would drop the process's POSIX
/// locks on it.
pub(crate) trait Backing: Read + Write + Seek {
    fn length(&self) -> io::Result<u64>;
    fn sync(&mut self) -> io::Result<()>;
}

impl Backing for File {
    fn length(&self) -> io::Result<u64> {
        Ok(self.metadata()?.len())
    }
    fn sync(&mut self) -> io::Result<()> {
        self.flush()?;
        self.sync_all()
    }
}

impl Backing for &File {
    fn length(&self) -> io::Result<u64> {
        Ok(self.metadata()?.len())
    }
    fn sync(&mut self) -> io::Result<()> {
        self.flush()?;
        self.sync_all()
    }
}

pub(crate) struct Pager<'a> {
    file: Box<dyn Backing + 'a>,
    page_size: usize,
    page_count: u32,
    header: Vec<u8>,
    read_budget: Option<crate::read_limits::ReadBudget>,
}

impl<'a> Pager<'a> {
    pub(crate) fn new(file: impl Backing + 'a) -> Result<Self> {
        let mut file: Box<dyn Backing + 'a> = Box::new(file);
        let len = file.length()?;
        let mut head = [0u8; 24];
        file.seek(SeekFrom::Start(0))?;
        file.read_exact(&mut head)
            .map_err(|_| corrupt("Файл короче заголовка 1Cv8.1CD"))?;
        if &head[..8] != SIGNATURE {
            return Err(Error::Unsupported(
                "Это не файл информационной базы 1Cv8.1CD: нет сигнатуры 1CDBMSV8".into(),
            ));
        }
        let version: [u8; 4] = head[8..12].try_into().unwrap();
        if version != VERSION_838 {
            return Err(Error::Unsupported(format!(
                "Формат файловой базы {}.{}.{}.{} не поддерживается: ibcmd-rs читает только формат 8.3.8",
                version[0], version[1], version[2], version[3]
            )));
        }
        let page_count = u32::from_le_bytes(head[12..16].try_into().unwrap());
        let page_size = u32::from_le_bytes(head[20..24].try_into().unwrap()) as usize;
        if !(4096..=65536).contains(&page_size) || !page_size.is_power_of_two() {
            return Err(corrupt(format!("Недопустимый размер страницы {page_size}")));
        }
        if page_count < 3 || (page_count as u64) * (page_size as u64) > len {
            return Err(corrupt(format!(
                "Заголовок указывает {page_count} страниц по {page_size} байт, а файл занимает {len} байт"
            )));
        }
        let mut pager = Pager {
            file,
            page_size,
            page_count,
            header: Vec::new(),
            read_budget: None,
        };
        pager.header = pager.read_page(0)?;
        Ok(pager)
    }

    pub(crate) fn set_read_budget(&mut self, budget: Option<crate::read_limits::ReadBudget>) {
        self.read_budget = budget;
    }
    pub(crate) fn read_is_bounded(&self) -> bool {
        self.read_budget.is_some()
    }
    pub(crate) fn check_read(&self) -> Result<()> {
        self.read_budget
            .as_ref()
            .map_or(Ok(()), |budget| budget.check())
    }
    pub(crate) fn materialize_object(&mut self, bytes: u64) -> Result<()> {
        self.read_budget
            .as_mut()
            .map_or(Ok(()), |budget| budget.object(bytes))
    }
    pub(crate) fn materialize_record(&mut self, bytes: u64) -> Result<()> {
        self.read_budget
            .as_mut()
            .map_or(Ok(()), |budget| budget.record(bytes))
    }

    pub(crate) fn page_size(&self) -> usize {
        self.page_size
    }

    pub(crate) fn page_count(&self) -> u32 {
        self.page_count
    }

    pub(crate) fn check_page(&self, page: u32, what: &str) -> Result<()> {
        if page == 0 || page >= self.page_count {
            return Err(corrupt(format!(
                "{what}: страница {page} вне файла из {} страниц",
                self.page_count
            )));
        }
        Ok(())
    }

    pub(crate) fn read_page(&mut self, page: u32) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; self.page_size];
        self.read_page_into(page, &mut buf)?;
        Ok(buf)
    }

    pub(crate) fn read_page_into(&mut self, page: u32, buf: &mut [u8]) -> Result<()> {
        self.check_read()?;
        if page >= self.page_count {
            return Err(corrupt(format!("Страница {page} вне файла")));
        }
        self.file
            .seek(SeekFrom::Start(page as u64 * self.page_size as u64))?;
        self.file.read_exact(buf)?;
        Ok(())
    }

    pub(crate) fn write_page(&mut self, page: u32, data: &[u8]) -> Result<()> {
        self.check_read()?;
        debug_assert_eq!(data.len(), self.page_size);
        if page >= self.page_count {
            return Err(corrupt(format!("Запись за пределы файла: страница {page}")));
        }
        self.file
            .seek(SeekFrom::Start(page as u64 * self.page_size as u64))?;
        self.file.write_all(data)?;
        Ok(())
    }

    /// Appends a zero page at the end of the file and returns its number.
    /// The header page count is written by [`Pager::flush_header`].
    pub(crate) fn append_page(&mut self) -> Result<u32> {
        self.check_read()?;
        let page = self.page_count;
        if page == u32::MAX {
            return Err(Error::Refused(
                "Файл базы достиг предела числа страниц".into(),
            ));
        }
        self.page_count += 1;
        let zero = vec![0u8; self.page_size];
        self.write_page(page, &zero)?;
        Ok(page)
    }

    pub(crate) fn flush_header(&mut self) -> Result<()> {
        let mut header = self.header.clone();
        header[12..16].copy_from_slice(&self.page_count.to_le_bytes());
        self.write_page(0, &header)?;
        self.header = header;
        Ok(())
    }

    pub(crate) fn sync(&mut self) -> Result<()> {
        self.file.sync()?;
        Ok(())
    }
}
