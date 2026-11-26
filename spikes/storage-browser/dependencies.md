# SPEC-0023 locked dependency review — 2026-10-22

The operator approved these candidates and reviewed locked transitives before this
resume. Resolution used crates.io and pinned all versions/checksums in the root
and standalone harness locks. Before compilation, every downloaded registry
archive SHA-256 was recomputed and matched the harness lock (59 archives,
including native-only packages). Root native versions are unchanged. All listed
licenses are permissive; no new copyleft obligation was found. This review is
not a vulnerability audit or endorsement of the older experimental VFS.

The target feature tree was inspected using `cargo tree --manifest-path
spikes/storage-browser/Cargo.toml --target wasm32-unknown-unknown --offline
--locked -e features`. Runtime defaults of rusqlite/sqlite-wasm-rs are disabled;
no SQLCipher/sqlite3mc, bindgen, serde, npm or async executor is added.
Tokio is included by VFS with sync/default (no runtime feature); IndexedDB
helpers are unconditional dependencies but their VFS is never selected.
The wasm-bindgen companions resolve at the approved exact versions and match
CLI 0.2.104. Build/proc-macro dependencies are included below, not just runtime.

Source review: sqlite-wasm-rs `build.rs` compiles bundled SQLite and libc shims
using cc; it disables threading/loadable extensions and enables URI handling.
Its `shim.rs` installs a memory VFS by default. Production WASM open therefore
names `opfs-sahpool` explicitly; it cannot use that default. VFS `sahpool.rs`
uses worker StorageManager and sync handles, installs asynchronously and keeps
file mappings when `clear_on_init` is false. The install boolean means default
VFS. The harness uses only a fresh unique directory, one connection, no WAL,
no custom VFS and no unsafe code in project sources.

First C build failed because Apple Clang lacks the WASM backend. Existing
Homebrew LLVM 22.1.8 supplies wasm32 and llvm-ar; command-local target-specific
variables select it. No global installation/configuration was changed. A
partially downloaded unused official WASI SDK archive was never executed and
was removed from the task scratch directory.

| Package | Locked version | Manifest license | Verified archive SHA-256 |
| --- | --- | --- | --- |
| accessory | 2.1.1 | MIT OR Apache-2.0 | `cc86a613208ca7d144ca24f6ec49c5cdfa5c05a46f2f7ab5880ff99371f165de` |
| bitflags | 2.13.2 | MIT OR Apache-2.0 | `3ded4057c258ba199e2d26386d3af3780957ecaee6c4ef4041c6b4b8b97c0b06` |
| bumpalo | 3.20.3 | MIT OR Apache-2.0 | `72f5acc6cb2ba439de613abc23857ec3d78374d8ed5ac84e9d11336e87da8649` |
| cc | 1.9.0 | MIT OR Apache-2.0 | `877bf67ac8e52449eb2f9e98a344ea163beb70dd1f9eebe3cc5357dcfeaee4df` |
| cfg-if | 1.0.5 | MIT OR Apache-2.0 | `4e7648175b45a9a48536d676f68d918270699102aa8dab5496df06904c914600` |
| delegate-display | 3.0.1 | MIT | `54e3a499943fd5180aeffcb708164ffb22125ceeccb05491b5297981d37d6249` |
| derive_more | 2.1.1 | MIT | `d751e9e49156b02b44f9c1815bcb94b984cdcc4396ecc32521c739452808b134` |
| derive_more-impl | 2.1.1 | MIT | `799a97264921d8623a957f6c3b9011f3b5492f557bbb7a5a19b7fa6d06ba8dcb` |
| fallible-iterator | 0.3.0 | MIT/Apache-2.0 | `2acce4a10f12dc2fb14a218589d4f1f62ef011b2d0cc4b3cb1bba8e94da14649` |
| fallible-streaming-iterator | 0.1.9 | MIT/Apache-2.0 | `7360491ce676a36bf9bb3c56c1aa791658183a54d2744120f27285738d90465a` |
| fancy_constructor | 2.1.1 | MIT OR Apache-2.0 | `4a8211ab12c36b63c269e17873331dc1da196a3edb7a5c79015677ef972da34a` |
| find-msvc-tools | 0.1.15 | MIT OR Apache-2.0 | `1c66915cc5391097de7a2b0b972e8505aa33c0df401fd3a6811dbdb32881ba48` |
| foldhash | 0.2.0 | Zlib | `77ce24cb58228fbb8aa041425bb1050850ac19177686ea6e0f41a70416f56fdb` |
| hashbrown | 0.16.1 | MIT OR Apache-2.0 | `841d1cc9bed7f9236f321df977030373f4a4163ae1a7dbfe1a51a2c1a51d9100` |
| impartial-ord | 2.0.0 | MIT | `bc680e03d1a08bdc1c01b60e0c39723430ced63d8cabd93cee0449b6ba916a25` |
| indexed_db_futures | 0.6.4 | MIT | `69ff41758cbd104e91033bb53bc449bec7eea65652960c81eddf3fc146ecea19` |
| indexed_db_futures_macros_internal | 1.0.0 | MIT | `caeba94923b68f254abef921cea7e7698bf4675fdd89d7c58bf1ed885b49a27d` |
| itoa | 1.0.18 | MIT OR Apache-2.0 | `8f42a60cbdf9a97f5d2305f08a87dc4e09308d1276d28c869c684d7777685682` |
| js-sys | 0.3.81 | MIT OR Apache-2.0 | `ec48937a97411dcb524a265206ccd4c90bb711fca92b2792c407f268825b9305` |
| libsqlite3-sys | 0.38.2 | MIT | `f1d20bef17f513b9b3004532233187769cd072d790971f4e4da0e346eb6401e8` |
| log | 0.4.34 | MIT OR Apache-2.0 | `f9f8bd3e56ce4dfc153cf470fffbfa98c7620958b312ca5c3a4b8d5181fd13c6` |
| macroific | 2.0.0 | Apache-2.0 | `89f276537b4b8f981bf1c13d79470980f71134b7bdcc5e6e911e910e556b0285` |
| macroific | 3.1.0 | Apache-2.0 | `cc8ab95c98294764c0f1eacebdb5b027849201b71fbc383871fe991a1de7eb83` |
| macroific_attr_parse | 2.0.0 | Apache-2.0 | `ad4023761b45fcd36abed8fb7ae6a80456b0a38102d55e89a57d9a594a236be9` |
| macroific_attr_parse | 3.1.0 | Apache-2.0 | `bf9b7bdb92825e6a507392b12f0570ade17480be20f4ca951b9071b40deba1d1` |
| macroific_core | 2.0.0 | Apache-2.0 | `d0a7594d3c14916fa55bef7e9d18c5daa9ed410dd37504251e4b75bbdeec33e3` |
| macroific_core | 3.1.0 | Apache-2.0 | `edc880da2bc00d6d87f748805e242d670e248d1b9b6fcef3190503210b6cf056` |
| macroific_macro | 2.0.0 | Apache-2.0 | `4da6f2ed796261b0a74e2b52b42c693bb6dee1effba3a482c49592659f824b3b` |
| macroific_macro | 3.1.0 | Apache-2.0 | `201faf668c264432727fb64e44407bcfcf1f5f8153bf4ec248eb6bbe4a39b144` |
| once_cell | 1.21.4 | MIT OR Apache-2.0 | `9f7c3e4beb33f85d45ae3e3a1792185706c8e16d043238c593331cc7cd313b50` |
| pin-project-lite | 0.2.17 | Apache-2.0 OR MIT | `a89322df9ebe1c1578d689c92318e070967d1042b512afbe49518723f4e6d5cd` |
| pkg-config | 0.3.34 | MIT OR Apache-2.0 | `f6b464fbc74e149a392436b17d523f769e057cb6877f6a5c4618bc6f11800548` |
| proc-macro2 | 1.0.107 | MIT OR Apache-2.0 | `985e7ec9bb745e6ce6535b544d84d6cd6f7ad8bd711c398938ae983b91a766d9` |
| quote | 1.0.47 | MIT OR Apache-2.0 | `1fbf4db142a473a8d80c26bbf18454ed458bf8d26c8219c331daecfdbd079001` |
| rusqlite | 0.40.2 | MIT | `23f2a97da3e3873c73cb2a2e71b35c40ff95e0b1eefa8d72d8499a6928c3b5b3` |
| rustc_version | 0.4.1 | MIT OR Apache-2.0 | `cfcb3a22ef46e85b45de6ee7e79d063319ebb6594faafcf1c225ea92ab6e9b92` |
| rustversion | 1.0.23 | MIT OR Apache-2.0 | `cf54715a573b99ac80df0bc206da022bcd442c974952c7b9720069370852e21f` |
| sealed | 0.6.0 | MIT OR Apache-2.0 | `22f968c5ea23d555e670b449c1c5e7b2fc399fdaec1d304a17cd48e288abc107` |
| sealed | 0.7.0 | MIT OR Apache-2.0 | `9b68e2ea526d9fb32f23ca8894fb5da9e743f34c2f41701f0501dc8a25c4b343` |
| semver | 1.0.28 | MIT OR Apache-2.0 | `8a7852d02fc848982e0c167ef163aaff9cd91dc640ba85e263cb1ce46fae51cd` |
| shlex | 2.0.1 | MIT OR Apache-2.0 | `f8fadd59c855ef2080decdef8ff161eb6661b86933c9d82e5ba29dc602a55aba` |
| smallvec | 1.16.3 | MIT OR Apache-2.0 | `5b3dc8af474f516a851ff4bd12db780f948b9250ad37211e4eec0bccea54e01b` |
| sqlite-wasm-rs | 0.5.1 | MIT | `05e98301bf8b0540c7de45ecd760539b9c62f5772aed172f08efba597c11cd5d` |
| sqlite-wasm-vfs | 0.1.1 | MIT | `c0eaef67248b9c7ef44d71890a532978a012e201033ec560603b30226e6f6070` |
| syn | 2.0.119 | MIT OR Apache-2.0 | `872831b642d1a07999a962a351ed35b955ea2cfc8f3862091e2a240a84f17297` |
| syn | 3.0.9 | MIT OR Apache-2.0 | `d78c8dee4c7bf0e14673097256fed6142ce9d3b85a408189d07482442145823b` |
| thiserror | 2.0.21 | MIT OR Apache-2.0 | `09e52cb86a36cede5cb101bf8908837b3e4c6e5e59fe7fd85c23fb56200d189e` |
| thiserror-impl | 2.0.21 | MIT OR Apache-2.0 | `fe5197923287db20a58125f0bc85c062f7f2c892de97b18c356f9efb14b28524` |
| tokio | 1.54.1 | MIT | `b52fd3255258d26fac7264ab5a76b53c13367136dc011d3d6ad4f8ed070396a3` |
| unicode-ident | 1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0 | `a2c754d6c33795a1c324727428e5a7dedb5b06195f9890bdbcba760d3e246563` |
| unicode-xid | 0.2.6 | MIT OR Apache-2.0 | `ebc1c04c71510c7f702b52b7c350734c9ff1295c464a03335b00bb84fc54f853` |
| vcpkg | 0.2.15 | MIT/Apache-2.0 | `accd4ea62f7bb7a82fe23066fb0957d48ef677f6eeb8215f372f52e48bb32426` |
| wasm-bindgen | 0.2.104 | MIT OR Apache-2.0 | `c1da10c01ae9f1ae40cbfac0bac3b1e724b320abfcf52229f80b547c0d250e2d` |
| wasm-bindgen-backend | 0.2.104 | MIT OR Apache-2.0 | `671c9a5a66f49d8a47345ab942e2cb93c7d1d0339065d4f8139c486121b43b19` |
| wasm-bindgen-futures | 0.4.54 | MIT OR Apache-2.0 | `7e038d41e478cc73bae0ff9b36c60cff1c98b8f38f8d7e8061e79ee63608ac5c` |
| wasm-bindgen-macro | 0.2.104 | MIT OR Apache-2.0 | `7ca60477e4c59f5f2986c50191cd972e3a50d8a95603bc9434501cf156a9a119` |
| wasm-bindgen-macro-support | 0.2.104 | MIT OR Apache-2.0 | `9f07d2f20d4da7b26400c9f4a0511e6e0345b040694e8a75bd41d578fa4421d7` |
| wasm-bindgen-shared | 0.2.104 | MIT OR Apache-2.0 | `bad67dc8b2a1a6e5448428adec4c3e84c43e561d8c9ee8a9e5aabeb193ec41d1` |
| web-sys | 0.3.81 | MIT OR Apache-2.0 | `9367c417a924a74cae129e6a2ae3b47fabb1f8995595ab474029da749a8be120` |

## Resolved feature sets

```text
accessory 2.1.1: (none)
bitflags 2.13.2: (none)
bumpalo 3.20.3: default
cc 1.9.0: (none)
cfg-if 1.0.5: (none)
delegate-display 3.0.1: (none)
derive_more 2.1.1: as_ref, debug, deref, deref_mut, from, into
derive_more-impl 2.1.1: as_ref, debug, default, deref, deref_mut, from, into
fallible-iterator 0.3.0: alloc, default
fallible-streaming-iterator 0.1.9: (none)
fancy_constructor 2.1.1: (none)
find-msvc-tools 0.1.15: (none)
foldhash 0.2.0: (none)
hashbrown 0.16.1: default-hasher
impartial-ord 2.0.0: (none)
indexed_db_futures 0.6.4: (none)
indexed_db_futures_macros_internal 1.0.0: (none)
itoa 1.0.18: (none)
js-sys 0.3.81: default, std
libsqlite3-sys 0.38.2: bundled, bundled_bindings, cc, default, min_sqlite_version_3_34_1, pkg-config, vcpkg
log 0.4.34: (none)
macroific 2.0.0: attr_parse, attributed, generic-impl, module-prefix
macroific 3.1.0: attr_parse, full, generic-impl, module-prefix
macroific_attr_parse 2.0.0: (none)
macroific_attr_parse 3.1.0: full
macroific_core 2.0.0: attributed, generic-impl, module-prefix
macroific_core 3.1.0: generic-impl, module-prefix
macroific_macro 2.0.0: attr_parse
macroific_macro 3.1.0: attr_parse
once_cell 1.21.4: (none)
pin-project-lite 0.2.17: (none)
pkg-config 0.3.34: (none)
proc-macro2 1.0.107: default, proc-macro
quote 1.0.47: default, proc-macro
rusqlite 0.40.2: bundled, ffi-sqlite-wasm-rs, modern_sqlite
rustc_version 0.4.1: (none)
rustversion 1.0.23: (none)
sealed 0.6.0: (none)
sealed 0.7.0: (none)
semver 1.0.28: default, std
shlex 2.0.1: default, std
smallvec 1.16.3: (none)
sqlite-wasm-rs 0.5.1: (none)
sqlite-wasm-vfs 0.1.1: (none)
syn 2.0.119: clone-impls, default, derive, extra-traits, full, parsing, printing, proc-macro, visit, visit-mut
syn 3.0.9: clone-impls, default, derive, full, parsing, printing, proc-macro
thiserror 2.0.21: default, std
thiserror-impl 2.0.21: (none)
tokio 1.54.1: default, sync
unicode-ident 1.0.27: (none)
unicode-xid 0.2.6: default
vcpkg 0.2.15: (none)
wasm-bindgen 0.2.104: default, std
wasm-bindgen-backend 0.2.104: (none)
wasm-bindgen-futures 0.4.54: default, std
wasm-bindgen-macro 0.2.104: (none)
wasm-bindgen-macro-support 0.2.104: (none)
wasm-bindgen-shared 0.2.104: (none)
web-sys 0.3.81: DedicatedWorkerGlobalScope, DomException, DomStringList, Event, EventTarget, FileSystemDirectoryHandle, FileSystemFileHandle, FileSystemGetDirectoryOptions, FileSystemGetFileOptions, FileSystemHandle, FileSystemReadWriteOptions, FileSystemSyncAccessHandle, IdbDatabase, IdbFactory, IdbKeyRange, IdbObjectStore, IdbObjectStoreParameters, IdbOpenDbRequest, IdbRequest, IdbRequestReadyState, IdbTransaction, IdbTransactionMode, IdbVersionChangeEvent, MessageEvent, StorageManager, Window, Worker, WorkerGlobalScope, WorkerNavigator, console, default, std
```
