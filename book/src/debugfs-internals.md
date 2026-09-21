# Exposing Internals in the Debugfs

This chapter is for Jay developers. It describes how to make compositor state
visible in the filesystem that `jay debugfs` mounts (see
[`jay debugfs`](cli.md#jay-debugfs)).

The filesystem is not written by hand. Each directory is declared in a small
language at the top of a Rust file. A code generator turns the declaration into
a Rust trait, and the compositor state is exposed by implementing that trait.

## Workflow

1. Create a file whose name ends in `_g_fuse.rs` somewhere below `src/`, and
   add it as a module of its parent. By convention the file is called
   `<name>_dfs_g_fuse.rs` and sits in the directory of the type it describes.
   The file must start with a `/* ... */` comment that contains the
   declarations.
2. Run the generator:

   ```shell
   ~$ ./codegen/codegen.sh
   ```

   It requires a nightly `rustfmt`. For every `_g_fuse.rs` file it
   - appends, or updates, an `include!` block between the markers
     `// FUSE GENERATED START` and `// FUSE GENERATED STOP`,
   - writes a description of the directories to
     `build/fuse/generated/m_<hash>.rs`, where `<hash>` is derived from the
     path of the file,
   - updates the list of all such descriptions in `build/fuse/generated.rs`.

   Commit all of these files. Descriptions of deleted `_g_fuse.rs` files are
   removed.
3. `build.rs` reads the descriptions and writes the Rust code into `OUT_DIR`.
   The `include!` block pulls it into the file as a module named `generated`.
4. Implement the generated traits in the same file.

Run the generator again after every change to the declarations.

A minimal file looks like this:

```rust
/*
dir wl_buffer {
    @inherit dfs_object,
    width: reg,
    color: reg (opt),
}
 */
use crate::ifs::wl_buffer::WlBuffer;
use crate::ifs::wl_buffer::wl_buffer_dfs_g_fuse::generated::wl_buffer;
use crate::utils::str_fmt::StrCtx;
use crate::utils::str_fmt::StrFmt;

impl wl_buffer::Dir for WlBuffer {
    fn read_width(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.width.str_fmt(buf, ctx);
    }

    fn has_color(&self) -> bool {
        self.color.is_some()
    }

    fn read_color(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        if let Some(v) = &self.color {
            v.str_fmt(buf, ctx);
        }
    }
}

// FUSE GENERATED START
include!(concat!(
    env!("OUT_DIR"),
    "/fuse/m_8b91a6dac50768623cab19ffe8ee101561f7aa63e020634152381b318f7e2326.rs",
));
// FUSE GENERATED STOP
```

## Declarations

A file contains any number of directories:

```
dir <name> (<attributes>) {
    <entry>,
    <entry>,
    @inherit <other dir>,
}
```

The attribute list is optional. Directory attributes:

`abstract`
: The directory is never shown on its own. Only its trait is generated. It is
  used as the target of `@inherit`.

`global`
: The directory is generated into `crate::utils::fuse::fuse_globals` instead of
  the module of the file, so that other files can inherit from it. Global
  directory names must be unique across the crate. The global abstract
  directories `dfs_object`, `dfs_node`, `dfs_toplevel_node`, `dfs_global`, and
  `dfs_backend` contain the files shared by all Wayland objects, tree nodes,
  toplevels, globals, and backends.

Each entry has the form `<name>: <type> (<attributes>)`. The name is the file
name and must be unique in the directory, including inherited entries. The
types are:

`reg`
: A regular file. Its contents are produced by `read_<name>`.

`link`
: A symbolic link. Its target is produced by `readlink_<name>`.

`view`
: A subdirectory, or any other inode, whose behavior is defined by a type that
  implements `FuseView`. The type is chosen in the implementation through the
  associated type `View<Name>`.

`custom`
: An arbitrary inode returned by `get_<name>`. This is used to embed a directory
  that is implemented elsewhere, for example the directory of a tree node.

Entry attributes:

`opt`
: The entry does not always exist.

`other`
: Only for `view`. The view is implemented by a different object than the
  directory itself. That object is returned by `get_<name>`.

`key = <n>`
: Only for `view` and `custom`. The inode is created with the fixed key `n`.
  See [Keys](#keys).

`no_timeout`
: Overrides the entry timeout. See [Entry Timeouts](#entry-timeouts).

`@inherit <dir>` adds all entries of another directory, which is either defined
in the same file or global.

## The Generated Trait

For each directory `<dir>` the generator creates a module
`generated::<dir>`, which contains a trait `Dir` and, unless the directory is
abstract, a type `View` that implements `FuseView<T>` for every `T: Dir`.
`Dir` requires `GetLiveness` (see [Liveness](#liveness)). If the directory
inherits from other directories, their traits are supertraits of `Dir` and
their entries do not appear in `Dir` itself.

The methods depend on the type and attributes of each entry:

`reg`
: `fn read_<name>(&self, buf: &mut String, ctx: &StrCtx<'_>)`

`link`
: `fn readlink_<name>(&self, depth: u64, buf: &mut String)`

`view`
: `type View<Name>: FuseView<Self>`

  Without `key`, additionally `fn keyof_<name>(&self, key: u64) -> u64`.

`view (other)`
: `type Base<Name>: GetLiveness`, `type View<Name>: FuseView<Self::Base<Name>>`,
  and `fn get_<name>(self: &Rc<Self>, key: u64) -> Rc<Self::Base<Name>>`

  Without `key`, additionally `fn keyof_<name>(&self, key: u64) -> u64`.

`custom`
: `fn get_<name>(self: &Rc<Self>, key: u64) -> FuseInodeWithKey`

`custom (key = n)`
: `fn get_<name>(self: &Rc<Self>, key: u64) -> Rc<dyn FuseInode>`

`<Name>` is the entry name in camel case.

`opt` adds the following:

- For `reg` and `link`: `fn has_<name>(&self) -> bool`.
- For `view` without `other`: `fn has_<name>(&self, key: u64) -> bool`.
- For `view (other)` and `custom`: `get_<name>` returns an `Option`.

The entry exists if `has_<name>` returns true or `get_<name>` returns `Some`.
`read_<name>` and `readlink_<name>` can still be called after `has_<name>` has
returned false, because the state can change between the two calls. They must
then write nothing and must not panic.

A `view` without `other` is implemented by the same object as the directory.
This is how a directory is split into subdirectories. For example, the `pending`
subdirectory of a surface is declared as `pending: view (key = 0)` and is
implemented by `WlSurface` itself:

```rust
impl wl_surface::Dir for WlSurface {
    type ViewPending = pending::View;
    // ...
}

impl pending::Dir for WlSurface {
    // ...
}
```

## Keys

Every inode consists of an object and a 64-bit key. The key lets one object back
several inodes. For example, the directory that describes a Wayland object
without dedicated debugfs support is implemented by the `Client` and uses the
object ID as the key.

Most directories ignore the key and are created with key 0. Entries with
`key = <n>` use that key. For other `view` entries, `keyof_<name>` maps the key
of the directory to the key of the child. The `key` argument of the generated
methods is the key of the directory.

## Entry Timeouts

The kernel caches the result of looking up a name. The generator sets the cache
duration from the entry:

- Entries that always exist, that is `reg`, `link`, and `view` with a fixed key
  and without `opt` or `other`, are cached without a time limit.
- All other entries are cached for 100 milliseconds.

`no_timeout` makes an entry cached without a time limit, even if it is
optional. Use it when the existence of the entry is decided when the object is
created and never changes. `wl_buffer` uses it for `dmabuf_device`, which
exists only for buffers created from a dmabuf.

The contents of files are not cached.

## Liveness

The filesystem identifies an inode by the address of its object, the Rust type
of its view, its key, and its parent and name. The inode cache holds a `Weak`
reference to the object, which keeps the address reserved, so it cannot be
reused while the inode is cached.

Dead inodes are found by a separate thread, which cannot access the `Weak`.
Instead, every object that backs an inode has a `Liveness`, which is updated
atomically when the object is dropped. The thread uses it to find inodes whose
`Weak` can no longer be upgraded. The compositor thread then removes those
inodes from the cache and drops the `Weak`.

Add a field and derive `GetLiveness`:

```rust
#[derive(GetLiveness)]
pub struct WlBuffer {
    // ...
    liveness: Liveness,
}
```

Initialize it with `Default::default()`. If the field has a different name,
annotate it with `#[liveness]`.

## Connecting a Directory

A directory is visible only if an existing directory refers to it. To turn an
`Rc<T>` into an inode, wrap it in a view:

```rust
use crate::utils::fuse::fuse_inode::FuseInodeExt;
use crate::utils::type_view::TypeViewExt1;

self.tv_wrap_rc::<wl_buffer::View>().without_key()
```

This returns a `FuseInodeWithKey`, which is what `custom` entries and several
of the collection views below expect.

Wayland objects appear in `clients/<client>/objects/<id>`. By default the
`Object` derive shows a directory with the ID, interface, and version. To
replace it, add `#[debugfs]` to the struct and implement `ObjectDebugfs`:

```rust
#[derive(Object, GetLiveness)]
#[debugfs]
pub struct WlBuffer {
    // ...
}

impl ObjectDebugfs for WlBuffer {
    fn object_debugfs(self: Rc<Self>, _client: &Rc<Client>) -> FuseInodeWithKey {
        self.debugfs()
    }
}
```

where `debugfs` is a function in the `_g_fuse.rs` file that performs the
wrapping shown above. The directory should inherit `dfs_object`, which is
implemented for all objects.

Tree nodes are connected through `Node::node_debugfs`. Directories of other
types need a `view (other)` or `custom` entry in a directory that is already
connected, for example in the `root` directory in
`src/state/state_dfs_g_fuse.rs`.

## Links

Link targets are relative paths. `depth` is the depth of the link below the root
of the filesystem, so a target that starts with `depth - 1` times `../` is
relative to the root. `src/dfs/dfs_helpers.rs` contains functions that write
such targets:

- `format_object_link` links to a Wayland object of a client.
- `format_client_link`, `format_output_link`, and `format_workspace_link` link
  to a client, an output, or a workspace.
- `write_root_link` writes only the prefix.

```rust
fn readlink_viewport(&self, depth: u64, buf: &mut String) {
    if let Some(obj) = self.viewporter.get() {
        format_object_link(buf, depth, self.client.id, obj.id());
    }
}
```

Prefer a link over a file that contains a flag or an ID when the target has a
directory of its own. An optional link carries the same information as a
`has_*` file.

## Formatting Values

`read_*` methods write through the `StrFmt` trait. It supports two formats:
human-readable text, which is used when the filesystem is mounted, and JSON,
which is used by `jay --json debugfs snapshot`. The format is available as
`ctx.fmt`. In the human-readable format a newline is appended to every file
automatically.

`StrFmt` is implemented for integers, floats, `bool`, `str`, `Option`, slices,
arrays, pairs, `Rc`, `Box`, and many compositor types. An `Option`
that is `None` is written as `nil` or `null`.

Enums implement `StaticText` to give each variant a name, and `read_*` writes
`self.value.get().text()`. Names are lower case with underscores.

Structs derive `StrFmt`. Each field is written as `name: value` in the
human-readable format and as a JSON object otherwise. Two attributes change the
output:

- `#[str_fmt(skip)]` on a field omits it.
- `#[str_fmt(transparent)]` on the struct writes the only field that is not
  skipped as if it were the whole struct. This is used for newtypes.

```rust
#[derive(StrFmt)]
#[str_fmt(transparent)]
pub struct ColorMatrix<To = Local, From = Local>(
    pub [[F64; 4]; 3],
    #[str_fmt(skip)] PhantomData<(To, From)>,
);
```

To write values that do not exist as a struct, such as a width and a height
stored separately, declare a local struct that derives `StrFmt` and write an
instance of it. Do not implement `StrFmt` by hand for structs.

## Collection Views

`src/utils/fuse/fuse_views.rs` and `src/dfs/dfs_helpers.rs` contain `FuseView`
implementations that are used as the `View<Name>` type of `view` entries. Each
of them is configured by a marker type `V` that implements a trait:

`FuseReg<V>`
: A regular file. `V` implements `FuseRegView`.

`FuseLink<V>`
: A symbolic link. `V` implements `FuseLinkView`.

`IterDir<V>`
: One entry per element produced by `iter`, looked up by `get`. `V` implements
  `IterDirView`.

`IterDirKeyed<V>`
: Like `IterDir`, but each element has its own key. `V` implements
  `IterDirKeyedView`.

`IterDirDyn<V>`
: Like `IterDir`, but each element is an arbitrary `FuseInodeWithKey`. `V`
  implements `IterDirDynView`.

`CopyHashMapDir<V>`, `CopyHashMapDir2<V>`
: The values of a `CopyHashMap`. `V` implements `CopyHashMapDirView` or
  `CopyHashMapDir2View`.

`HashMapDir<V>`
: The values of a `HashMap`. `V` implements `HashMapDirView`.

`BinarySearchMapDir<V>`
: The values of a binary search map. `V` implements `BinarySearchMapDirView`.

`DevTDir<V>`
: Devices named `major:minor`. `V` implements `DevTDirView`.

`DfsObjectLinkDir<V>`, `DfsCopyHashMapObjectLinkDir<V>`
: Links to Wayland objects of one client. `V` implements `DfsObjectLinkDirView`
  or `DfsCopyHashMapObjectLinkDirView`.

`ClientObjectDir<V>`
: Links to Wayland objects of several clients, named `client:object`. `V`
  implements `ClientObjectDirView`.

The elements of these collections must implement `GetLiveness`.

A generated `View` can also be used as the element view of a collection. For
example, `IterDirKeyed<ColorDescriptions>` in `src/state/state_dfs_g_fuse.rs`
lists all color descriptions as regular files named after their IDs.

## Stability

The layout of the filesystem is not a stable interface. Entries can be renamed
or removed freely.
