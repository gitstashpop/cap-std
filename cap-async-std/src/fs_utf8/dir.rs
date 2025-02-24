use crate::fs::{OpenOptions, Permissions};
use crate::fs_utf8::{from_utf8, to_utf8, DirBuilder, File, Metadata, ReadDir};
use async_std::{fs, io};
use camino::{Utf8Path, Utf8PathBuf};
use cap_primitives::AmbientAuthority;
use io_lifetimes::raw::{AsRawFilelike, FromRawFilelike};
use io_lifetimes::AsFilelike;
#[cfg(not(windows))]
use io_lifetimes::{AsFd, BorrowedFd, OwnedFd};
#[cfg(windows)]
use io_lifetimes::{AsHandle, BorrowedHandle, OwnedHandle};
use std::fmt;
use std::mem::ManuallyDrop;
#[cfg(unix)]
use {
    crate::os::unix::net::{UnixDatagram, UnixListener, UnixStream},
    async_std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd},
};
#[cfg(windows)]
use {
    async_std::os::windows::io::{AsRawHandle, FromRawHandle, IntoRawHandle, RawHandle},
    io_extras::os::windows::{
        AsHandleOrSocket, AsRawHandleOrSocket, BorrowedHandleOrSocket, IntoRawHandleOrSocket,
        OwnedHandleOrSocket, RawHandleOrSocket,
    },
};

/// A reference to an open directory on a filesystem.
///
/// This does not directly correspond to anything in `async_std`, however its
/// methods correspond to the [functions in `async_std::fs`] and the
/// constructor methods for [`async_std::fs::File`].
///
/// Unlike `async_std::fs`, this API's `canonicalize` returns a relative path
/// since absolute paths don't interoperate well with the capability model.
///
/// [functions in `async_std::fs`]: https://docs.rs/async-std/latest/async_std/fs/index.html#functions
#[derive(Clone)]
pub struct Dir {
    cap_std: crate::fs::Dir,
}

impl Dir {
    /// Constructs a new instance of `Self` from the given
    /// `async_std::fs::File`.
    ///
    /// To prevent race conditions on Windows, the file must be opened without
    /// `FILE_SHARE_DELETE`.
    ///
    /// This grants access the resources the `async_std::fs::File` instance
    /// already has access to.
    #[inline]
    pub fn from_std_file(std_file: fs::File) -> Self {
        Self::from_cap_std(crate::fs::Dir::from_std_file(std_file))
    }

    /// Constructs a new instance of `Self` from the given `cap_std::fs::Dir`.
    #[inline]
    pub fn from_cap_std(cap_std: crate::fs::Dir) -> Self {
        Self { cap_std }
    }

    /// Attempts to open a file in read-only mode.
    ///
    /// This corresponds to [`async_std::fs::File::open`], but only accesses
    /// paths relative to `self`.
    #[inline]
    pub async fn open<P: AsRef<Utf8Path>>(&self, path: P) -> io::Result<File> {
        let path = from_utf8(path.as_ref())?;
        self.cap_std.open(path).await.map(File::from_cap_std)
    }

    /// Opens a file at `path` with the options specified by `options`.
    ///
    /// This corresponds to [`async_std::fs::OpenOptions::open`].
    ///
    /// Instead of being a method on `OpenOptions`, this is a method on `Dir`,
    /// and it only accesses paths relative to `self`.
    #[inline]
    pub async fn open_with<P: AsRef<Utf8Path>>(
        &self,
        path: P,
        options: &OpenOptions,
    ) -> io::Result<File> {
        let path = from_utf8(path.as_ref())?;
        self.cap_std
            .open_with(path, options)
            .await
            .map(File::from_cap_std)
    }

    /// Attempts to open a directory.
    #[inline]
    pub async fn open_dir<P: AsRef<Utf8Path>>(&self, path: P) -> io::Result<Self> {
        let path = from_utf8(path.as_ref())?;
        self.cap_std.open_dir(path).await.map(Self::from_cap_std)
    }

    /// Creates a new, empty directory at the provided path.
    ///
    /// This corresponds to [`async_std::fs::create_dir`], but only accesses
    /// paths relative to `self`.
    ///
    /// TODO: async: fix this when we fix https://github.com/bytecodealliance/cap-std/issues/51
    #[inline]
    pub fn create_dir<P: AsRef<Utf8Path>>(&self, path: P) -> io::Result<()> {
        let path = from_utf8(path.as_ref())?;
        self.cap_std.create_dir(path)
    }

    /// Recursively create a directory and all of its parent components if they
    /// are missing.
    ///
    /// This corresponds to [`async_std::fs::create_dir_all`], but only
    /// accesses paths relative to `self`.
    ///
    /// TODO: async: fix this when we fix https://github.com/bytecodealliance/cap-std/issues/51
    #[inline]
    pub fn create_dir_all<P: AsRef<Utf8Path>>(&self, path: P) -> io::Result<()> {
        let path = from_utf8(path.as_ref())?;
        self.cap_std.create_dir_all(path)
    }

    /// Creates the specified directory with the options configured in this
    /// builder.
    ///
    /// This corresponds to [`async_std::fs::DirBuilder::create`].
    ///
    /// TODO: async: fix this when we fix https://github.com/bytecodealliance/cap-std/issues/51
    #[inline]
    pub fn create_dir_with<P: AsRef<Utf8Path>>(
        &self,
        path: P,
        dir_builder: &DirBuilder,
    ) -> io::Result<()> {
        let path = from_utf8(path.as_ref())?;
        self.cap_std.create_dir_with(path, dir_builder)
    }

    /// Opens a file in write-only mode.
    ///
    /// This corresponds to [`async_std::fs::File::create`], but only accesses
    /// paths relative to `self`.
    #[inline]
    pub async fn create<P: AsRef<Utf8Path>>(&self, path: P) -> io::Result<File> {
        let path = from_utf8(path.as_ref())?;
        self.cap_std.create(path).await.map(File::from_cap_std)
    }

    /// Returns the canonical form of a path with all intermediate components
    /// normalized and symbolic links resolved.
    ///
    /// This corresponds to [`async_std::fs::canonicalize`], but instead of
    /// returning an absolute path, returns a path relative to the
    /// directory represented by `self`.
    #[inline]
    pub async fn canonicalize<P: AsRef<Utf8Path>>(&self, path: P) -> io::Result<Utf8PathBuf> {
        let path = from_utf8(path.as_ref())?;
        self.cap_std.canonicalize(path).await.and_then(to_utf8)
    }

    /// Copies the contents of one file to another. This function will also
    /// copy the permission bits of the original file to the destination
    /// file.
    ///
    /// This corresponds to [`async_std::fs::copy`], but only accesses paths
    /// relative to `self`.
    #[inline]
    pub async fn copy<P: AsRef<Utf8Path>, Q: AsRef<Utf8Path>>(
        &self,
        from: P,
        to_dir: &Self,
        to: Q,
    ) -> io::Result<u64> {
        let from = from_utf8(from.as_ref())?;
        let to = from_utf8(to.as_ref())?;
        self.cap_std.copy(from, &to_dir.cap_std, to).await
    }

    /// Creates a new hard link on a filesystem.
    ///
    /// This corresponds to [`async_std::fs::hard_link`], but only accesses
    /// paths relative to `self`.
    #[inline]
    pub async fn hard_link<P: AsRef<Utf8Path>, Q: AsRef<Utf8Path>>(
        &self,
        src: P,
        dst_dir: &Self,
        dst: Q,
    ) -> io::Result<()> {
        let src = from_utf8(src.as_ref())?;
        let dst = from_utf8(dst.as_ref())?;
        self.cap_std.hard_link(src, &dst_dir.cap_std, dst).await
    }

    /// Given a path, query the file system to get information about a file,
    /// directory, etc.
    ///
    /// This corresponds to [`async_std::fs::metadata`], but only accesses
    /// paths relative to `self`.
    #[inline]
    pub async fn metadata<P: AsRef<Utf8Path>>(&self, path: P) -> io::Result<Metadata> {
        let path = from_utf8(path.as_ref())?;
        self.cap_std.metadata(path).await
    }

    /// Queries metadata about the underlying directory.
    ///
    /// This is similar to [`std::fs::File::metadata`], but for `Dir` rather
    /// than for `File`.
    #[inline]
    pub async fn dir_metadata(&self) -> io::Result<Metadata> {
        self.cap_std.dir_metadata().await
    }

    /// Returns an iterator over the entries within `self`.
    #[inline]
    pub async fn entries(&self) -> io::Result<ReadDir> {
        self.cap_std.entries().await.map(ReadDir::from_cap_std)
    }

    /// Returns an iterator over the entries within a directory.
    ///
    /// This corresponds to [`async_std::fs::read_dir`], but only accesses
    /// paths relative to `self`.
    #[inline]
    pub async fn read_dir<P: AsRef<Utf8Path>>(&self, path: P) -> io::Result<ReadDir> {
        let path = from_utf8(path.as_ref())?;
        self.cap_std.read_dir(path).await.map(ReadDir::from_cap_std)
    }

    /// Read the entire contents of a file into a bytes vector.
    ///
    /// This corresponds to [`async_std::fs::read`], but only accesses paths
    /// relative to `self`.
    #[inline]
    pub async fn read<P: AsRef<Utf8Path>>(&self, path: P) -> io::Result<Vec<u8>> {
        let path = from_utf8(path.as_ref())?;
        self.cap_std.read(path).await
    }

    /// Reads a symbolic link, returning the file that the link points to.
    ///
    /// This corresponds to [`async_std::fs::read_link`], but only accesses
    /// paths relative to `self`.
    #[inline]
    pub async fn read_link<P: AsRef<Utf8Path>>(&self, path: P) -> io::Result<Utf8PathBuf> {
        let path = from_utf8(path.as_ref())?;
        self.cap_std.read_link(path).await.and_then(to_utf8)
    }

    /// Read the entire contents of a file into a string.
    ///
    /// This corresponds to [`async_std::fs::read_to_string`], but only
    /// accesses paths relative to `self`.
    #[inline]
    pub async fn read_to_string<P: AsRef<Utf8Path>>(&self, path: P) -> io::Result<String> {
        let path = from_utf8(path.as_ref())?;
        self.cap_std.read_to_string(path).await
    }

    /// Removes an empty directory.
    ///
    /// This corresponds to [`async_std::fs::remove_dir`], but only accesses
    /// paths relative to `self`.
    #[inline]
    pub async fn remove_dir<P: AsRef<Utf8Path>>(&self, path: P) -> io::Result<()> {
        let path = from_utf8(path.as_ref())?;
        self.cap_std.remove_dir(path).await
    }

    /// Removes a directory at this path, after removing all its contents. Use
    /// carefully!
    ///
    /// This corresponds to [`async_std::fs::remove_dir_all`], but only
    /// accesses paths relative to `self`.
    #[inline]
    pub async fn remove_dir_all<P: AsRef<Utf8Path>>(&self, path: P) -> io::Result<()> {
        let path = from_utf8(path.as_ref())?;
        self.cap_std.remove_dir_all(path).await
    }

    /// Remove the directory referenced by `self` and consume `self`.
    ///
    /// Even though this implementation works in terms of handles as much as
    /// possible, removal is not guaranteed to be atomic with respect to a
    /// concurrent rename of the directory.
    #[inline]
    pub async fn remove_open_dir(self) -> io::Result<()> {
        self.cap_std.remove_open_dir().await
    }

    /// Removes the directory referenced by `self`, after removing all its
    /// contents, and consume `self`. Use carefully!
    ///
    /// Even though this implementation works in terms of handles as much as
    /// possible, removal is not guaranteed to be atomic with respect to a
    /// concurrent rename of the directory.
    #[inline]
    pub async fn remove_open_dir_all(self) -> io::Result<()> {
        self.cap_std.remove_open_dir_all().await
    }

    /// Removes a file from a filesystem.
    ///
    /// This corresponds to [`async_std::fs::remove_file`], but only accesses
    /// paths relative to `self`.
    #[inline]
    pub async fn remove_file<P: AsRef<Utf8Path>>(&self, path: P) -> io::Result<()> {
        let path = from_utf8(path.as_ref())?;
        self.cap_std.remove_file(path).await
    }

    /// Rename a file or directory to a new name, replacing the original file
    /// if to already exists.
    ///
    /// This corresponds to [`async_std::fs::rename`], but only accesses paths
    /// relative to `self`.
    #[inline]
    pub async fn rename<P: AsRef<Utf8Path>, Q: AsRef<Utf8Path>>(
        &self,
        from: P,
        to_dir: &Self,
        to: Q,
    ) -> io::Result<()> {
        let from = from_utf8(from.as_ref())?;
        let to = from_utf8(to.as_ref())?;
        self.cap_std.rename(from, &to_dir.cap_std, to).await
    }

    /// Changes the permissions found on a file or a directory.
    ///
    /// This corresponds to [`async_std::fs::set_permissions`], but only
    /// accesses paths relative to `self`. Also, on some platforms, this
    /// function may fail if the file or directory cannot be opened for
    /// reading or writing first.
    pub async fn set_permissions<P: AsRef<Utf8Path>>(
        &self,
        path: P,
        perm: Permissions,
    ) -> io::Result<()> {
        let path = from_utf8(path.as_ref())?;
        self.cap_std.set_permissions(path, perm).await
    }

    /// Query the metadata about a file without following symlinks.
    ///
    /// This corresponds to [`async_std::fs::symlink_metadata`], but only
    /// accesses paths relative to `self`.
    #[inline]
    pub async fn symlink_metadata<P: AsRef<Utf8Path>>(&self, path: P) -> io::Result<Metadata> {
        let path = from_utf8(path.as_ref())?;
        self.cap_std.symlink_metadata(path).await
    }

    /// Write a slice as the entire contents of a file.
    ///
    /// This corresponds to [`async_std::fs::write`], but only accesses paths
    /// relative to `self`.
    #[inline]
    pub async fn write<P: AsRef<Utf8Path>, C: AsRef<[u8]>>(
        &self,
        path: P,
        contents: C,
    ) -> io::Result<()> {
        let path = from_utf8(path.as_ref())?;
        self.cap_std.write(path, contents).await
    }

    /// Creates a new symbolic link on a filesystem.
    ///
    /// The `original` argument provides the target of the symlink. The `link`
    /// argument provides the name of the created symlink.
    ///
    /// Despite the argument ordering, `original` is not resolved relative to
    /// `self` here. `link` is resolved relative to `self`, and `original` is
    /// not resolved within this function.
    ///
    /// The `link` path is resolved when the symlink is dereferenced, relative
    /// to the directory that contains it.
    ///
    /// This corresponds to [`async_std::os::unix::fs::symlink`], but only
    /// accesses paths relative to `self`.
    ///
    /// [`async_std::os::unix::fs::symlink`]: https://docs.rs/async-std/latest/async_std/os/unix/fs/fn.symlink.html
    #[cfg(not(windows))]
    #[inline]
    pub async fn symlink<P: AsRef<Utf8Path>, Q: AsRef<Utf8Path>>(
        &self,
        original: P,
        link: Q,
    ) -> io::Result<()> {
        let original = from_utf8(original.as_ref())?;
        let link = from_utf8(link.as_ref())?;
        self.cap_std.symlink(original, link).await
    }

    /// Creates a new file symbolic link on a filesystem.
    ///
    /// The `original` argument provides the target of the symlink. The `link`
    /// argument provides the name of the created symlink.
    ///
    /// Despite the argument ordering, `original` is not resolved relative to
    /// `self` here. `link` is resolved relative to `self`, and `original` is
    /// not resolved within this function.
    ///
    /// The `link` path is resolved when the symlink is dereferenced, relative
    /// to the directory that contains it.
    ///
    /// This corresponds to [`async_std::os::windows::fs::symlink_file`], but
    /// only accesses paths relative to `self`.
    ///
    /// [`async_std::os::windows::fs::symlink_file`]: https://docs.rs/async-std/latest/async_std/os/windows/fs/fn.symlink_file.html
    #[cfg(windows)]
    #[inline]
    pub async fn symlink_file<P: AsRef<Utf8Path>, Q: AsRef<Utf8Path>>(
        &self,
        original: P,
        link: Q,
    ) -> io::Result<()> {
        let original = from_utf8(original.as_ref())?;
        let link = from_utf8(link.as_ref())?;
        self.cap_std.symlink_file(original, link).await
    }

    /// Creates a new directory symlink on a filesystem.
    ///
    /// The `original` argument provides the target of the symlink. The `link`
    /// argument provides the name of the created symlink.
    ///
    /// Despite the argument ordering, `original` is not resolved relative to
    /// `self` here. `link` is resolved relative to `self`, and `original` is
    /// not resolved within this function.
    ///
    /// The `link` path is resolved when the symlink is dereferenced, relative
    /// to the directory that contains it.
    ///
    /// This corresponds to [`async_std::os::windows::fs::symlink_dir`], but
    /// only accesses paths relative to `self`.
    ///
    /// [`async_std::os::windows::fs::symlink_dir`]: https://docs.rs/async-std/latest/async_std/os/windows/fs/fn.symlink_dir.html
    #[cfg(windows)]
    #[inline]
    pub async fn symlink_dir<P: AsRef<Utf8Path>, Q: AsRef<Utf8Path>>(
        &self,
        original: P,
        link: Q,
    ) -> io::Result<()> {
        let original = from_utf8(original.as_ref())?;
        let link = from_utf8(link.as_ref())?;
        self.cap_std.symlink_dir(original, link).await
    }

    /// Creates a new `UnixListener` bound to the specified socket.
    ///
    /// This corresponds to [`async_std::os::unix::net::UnixListener::bind`],
    /// but only accesses paths relative to `self`.
    ///
    /// XXX: This function is not yet implemented.
    ///
    /// [`async_std::os::unix::net::UnixListener::bind`]: https://docs.rs/async-std/latest/async_std/os/unix/net/struct.UnixListener.html#method.bind
    #[doc(alias = "bind")]
    #[cfg(unix)]
    #[inline]
    pub async fn bind_unix_listener<P: AsRef<Utf8Path>>(
        &self,
        path: P,
    ) -> io::Result<UnixListener> {
        let path = from_utf8(path.as_ref())?;
        self.cap_std.bind_unix_listener(path).await
    }

    /// Connects to the socket named by path.
    ///
    /// This corresponds to [`async_std::os::unix::net::UnixStream::connect`],
    /// but only accesses paths relative to `self`.
    ///
    /// XXX: This function is not yet implemented.
    ///
    /// [`async_std::os::unix::net::UnixStream::connect`]: https://docs.rs/async-std/latest/async_std/os/unix/net/struct.UnixStream.html#method.connect
    #[doc(alias = "connect")]
    #[cfg(unix)]
    #[inline]
    pub async fn connect_unix_stream<P: AsRef<Utf8Path>>(&self, path: P) -> io::Result<UnixStream> {
        let path = from_utf8(path.as_ref())?;
        self.cap_std.connect_unix_stream(path).await
    }

    /// Creates a Unix datagram socket bound to the given path.
    ///
    /// This corresponds to [`async_std::os::unix::net::UnixDatagram::bind`],
    /// but only accesses paths relative to `self`.
    ///
    /// XXX: This function is not yet implemented.
    ///
    /// [`async_std::os::unix::net::UnixDatagram::bind`]: https://docs.rs/async-std/latest/async_std/os/unix/net/struct.UnixDatagram.html#method.bind
    #[doc(alias = "bind")]
    #[cfg(unix)]
    #[inline]
    pub async fn bind_unix_datagram<P: AsRef<Utf8Path>>(
        &self,
        path: P,
    ) -> io::Result<UnixDatagram> {
        let path = from_utf8(path.as_ref())?;
        self.cap_std.bind_unix_datagram(path).await
    }

    /// Connects the socket to the specified address.
    ///
    /// This corresponds to
    /// [`async_std::os::unix::net::UnixDatagram::connect`], but only
    /// accesses paths relative to `self`.
    ///
    /// XXX: This function is not yet implemented.
    ///
    /// [`async_std::os::unix::net::UnixDatagram::connect`]: https://docs.rs/async-std/latest/async_std/os/unix/net/struct.UnixDatagram.html#method.connect
    #[doc(alias = "connect")]
    #[cfg(unix)]
    #[inline]
    pub async fn connect_unix_datagram<P: AsRef<Utf8Path>>(
        &self,
        unix_datagram: &UnixDatagram,
        path: P,
    ) -> io::Result<()> {
        let path = from_utf8(path.as_ref())?;
        self.cap_std
            .connect_unix_datagram(unix_datagram, path)
            .await
    }

    /// Sends data on the socket to the specified address.
    ///
    /// This corresponds to
    /// [`async_std::os::unix::net::UnixDatagram::send_to`], but only
    /// accesses paths relative to `self`.
    ///
    /// XXX: This function is not yet implemented.
    ///
    /// [`async_std::os::unix::net::UnixDatagram::send_to`]: https://docs.rs/async-std/latest/async_std/os/unix/net/struct.UnixDatagram.html#method.send_to
    #[doc(alias = "send_to")]
    #[cfg(unix)]
    #[inline]
    pub async fn send_to_unix_datagram_addr<P: AsRef<Utf8Path>>(
        &self,
        unix_datagram: &UnixDatagram,
        buf: &[u8],
        path: P,
    ) -> io::Result<usize> {
        let path = from_utf8(path.as_ref())?;
        self.cap_std
            .send_to_unix_datagram_addr(unix_datagram, buf, path)
            .await
    }

    // async_std doesn't have `try_clone`.

    /// Returns `true` if the path points at an existing entity.
    ///
    /// This corresponds to [`async_std::path::Path::exists`], but only
    /// accesses paths relative to `self`.
    #[inline]
    pub async fn exists<P: AsRef<Utf8Path>>(&self, path: P) -> bool {
        match from_utf8(path.as_ref()) {
            Ok(path) => self.cap_std.exists(path).await,
            Err(_) => false,
        }
    }

    /// Returns `true` if the path points at an existing entity.
    ///
    /// This corresponds to [`async_std::path::Path::exists`], but only
    /// accesses paths relative to `self`.
    #[inline]
    pub async fn try_exists<P: AsRef<Utf8Path>>(&self, path: P) -> io::Result<bool> {
        self.cap_std.try_exists(from_utf8(path.as_ref())?).await
    }

    /// Returns `true` if the path exists on disk and is pointing at a regular
    /// file.
    ///
    /// This corresponds to [`async_std::path::Path::is_file`], but only
    /// accesses paths relative to `self`.
    #[inline]
    pub async fn is_file<P: AsRef<Utf8Path>>(&self, path: P) -> bool {
        match from_utf8(path.as_ref()) {
            Ok(path) => self.cap_std.is_file(path).await,
            Err(_) => false,
        }
    }

    /// Checks if `path` is a directory.
    ///
    /// This is similar to [`async_std::path::Path::is_dir`] in that it checks
    /// if `path` relative to `Dir` is a directory. This function will traverse
    /// symbolic links to query information about the destination file. In case
    /// of broken symbolic links, this will return `false`.
    #[inline]
    pub async fn is_dir<P: AsRef<Utf8Path>>(&self, path: P) -> bool {
        match from_utf8(path.as_ref()) {
            Ok(path) => self.cap_std.is_dir(path).await,
            Err(_) => false,
        }
    }

    /// Constructs a new instance of `Self` by opening the given path as a
    /// directory using the host process' ambient authority.
    ///
    /// # Ambient Authority
    ///
    /// This function is not sandboxed and may access any path that the host
    /// process has access to.
    #[inline]
    pub async fn open_ambient_dir<P: AsRef<Utf8Path>>(
        path: P,
        ambient_authority: AmbientAuthority,
    ) -> io::Result<Self> {
        let path = from_utf8(path.as_ref())?;
        crate::fs::Dir::open_ambient_dir(path, ambient_authority)
            .await
            .map(Self::from_cap_std)
    }

    /// Constructs a new instance of `Self` by opening the parent directory
    /// (aka "..") of `self`, using the host process' ambient authority.
    ///
    /// # Ambient Authority
    ///
    /// This function accesses a directory outside of the `self` subtree.
    #[inline]
    pub async fn open_parent_dir(&self, ambient_authority: AmbientAuthority) -> io::Result<Self> {
        self.cap_std
            .open_parent_dir(ambient_authority)
            .await
            .map(Self::from_cap_std)
    }

    /// Recursively create a directory and all of its parent components if they
    /// are missing, using the host process' ambient authority.
    ///
    /// # Ambient Authority
    ///
    /// This function is not sandboxed and may access any path that the host
    /// process has access to.
    #[inline]
    pub async fn create_ambient_dir_all<P: AsRef<Utf8Path>>(
        path: P,
        ambient_authority: AmbientAuthority,
    ) -> io::Result<()> {
        let _ = ambient_authority;
        let path = from_utf8(path.as_ref())?;
        fs::create_dir_all(path).await
    }

    /// Construct a new instance of `Self` from existing directory file
    /// descriptor.
    ///
    /// This can be useful when interacting with other libraries and or C/C++
    /// code which has invoked `openat(..., O_DIRECTORY)` external to this
    /// crate.
    pub async fn reopen_dir<Filelike: AsFilelike>(dir: &Filelike) -> io::Result<Self> {
        // Our public API has a `&Filelike` here, which prevents us from doing
        // a `clone` as we usually do. So instead, we use the raw fd, which we
        // can clone and depend on it remaining open until we return.
        let raw_filelike = dir.as_filelike_view::<std::fs::File>().as_raw_filelike();
        // SAFETY: `raw_filelike` remains open for the duration of the `reopen_dir`
        // call.
        let file = ManuallyDrop::new(unsafe { std::fs::File::from_raw_filelike(raw_filelike) });
        crate::fs::Dir::reopen_dir(&*file)
            .await
            .map(Self::from_cap_std)
    }
}

// Safety: `FilelikeViewType` is implemented for `std::fs::File`.
unsafe impl io_lifetimes::views::FilelikeViewType for Dir {}

#[cfg(not(windows))]
impl FromRawFd for Dir {
    #[inline]
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        Self::from_std_file(fs::File::from_raw_fd(fd))
    }
}

#[cfg(not(windows))]
impl From<OwnedFd> for Dir {
    #[inline]
    fn from(fd: OwnedFd) -> Self {
        Self::from_std_file(fs::File::from(fd))
    }
}

#[cfg(windows)]
impl FromRawHandle for Dir {
    /// To prevent race conditions on Windows, the handle must be opened
    /// without `FILE_SHARE_DELETE`.
    #[inline]
    unsafe fn from_raw_handle(handle: RawHandle) -> Self {
        Self::from_std_file(fs::File::from_raw_handle(handle))
    }
}

#[cfg(windows)]
impl From<OwnedHandle> for Dir {
    #[inline]
    fn from(handle: OwnedHandle) -> Self {
        Self::from_std_file(fs::File::from(handle))
    }
}

#[cfg(not(windows))]
impl AsRawFd for Dir {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        self.cap_std.as_raw_fd()
    }
}

#[cfg(not(windows))]
impl AsFd for Dir {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.cap_std.as_fd()
    }
}

#[cfg(windows)]
impl AsRawHandle for Dir {
    #[inline]
    fn as_raw_handle(&self) -> RawHandle {
        self.cap_std.as_raw_handle()
    }
}

#[cfg(windows)]
impl AsHandle for Dir {
    #[inline]
    fn as_handle(&self) -> BorrowedHandle<'_> {
        self.cap_std.as_handle()
    }
}

#[cfg(windows)]
impl AsHandleOrSocket for Dir {
    #[inline]
    fn as_handle_or_socket(&self) -> BorrowedHandleOrSocket<'_> {
        self.cap_std.as_handle_or_socket()
    }
}

#[cfg(windows)]
impl AsRawHandleOrSocket for Dir {
    #[inline]
    fn as_raw_handle_or_socket(&self) -> RawHandleOrSocket {
        self.cap_std.as_raw_handle_or_socket()
    }
}

#[cfg(not(windows))]
impl IntoRawFd for Dir {
    #[inline]
    fn into_raw_fd(self) -> RawFd {
        self.cap_std.into_raw_fd()
    }
}

#[cfg(not(windows))]
impl From<Dir> for OwnedFd {
    #[inline]
    fn from(dir: Dir) -> OwnedFd {
        dir.cap_std.into()
    }
}

#[cfg(windows)]
impl IntoRawHandle for Dir {
    #[inline]
    fn into_raw_handle(self) -> RawHandle {
        self.cap_std.into_raw_handle()
    }
}

#[cfg(windows)]
impl From<Dir> for OwnedHandle {
    #[inline]
    fn from(dir: Dir) -> OwnedHandle {
        dir.cap_std.into()
    }
}

#[cfg(windows)]
impl IntoRawHandleOrSocket for Dir {
    #[inline]
    fn into_raw_handle_or_socket(self) -> RawHandleOrSocket {
        self.cap_std.into_raw_handle_or_socket()
    }
}

#[cfg(windows)]
impl From<Dir> for OwnedHandleOrSocket {
    #[inline]
    fn from(dir: Dir) -> Self {
        dir.cap_std.into()
    }
}

impl fmt::Debug for Dir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.cap_std.fmt(f)
    }
}
