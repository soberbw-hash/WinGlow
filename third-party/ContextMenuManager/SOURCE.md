# ContextMenuManager GUID identification dictionary

WinGlow imports only the unmodified `GuidInfosDic.ini` metadata from [BluePointLilac/ContextMenuManager](https://github.com/BluePointLilac/ContextMenuManager). No C# implementation, executable, command dictionary, XML registry-edit rules or artwork is incorporated.

- Pinned source commit: `55507155dd8e49c7ab4606da97f2af192d590dfe`.
- [Original dictionary](https://github.com/BluePointLilac/ContextMenuManager/blob/55507155dd8e49c7ab4606da97f2af192d590dfe/ContextMenuManager/Properties/Resources/Texts/GuidInfosDic.ini).
- SHA-256: `39a37d11bfa8066d2a9ab95895b165ff7397573f2cdb7d56a3bf81dc28ecb8b8`.
- Upstream author: BluePointLilac; current repository license: GPL-3.0, copied to `licenses/ContextMenuManager-Dictionary-GPL-3.0.txt`. Old screenshots describing MIT do not determine the current file's license. This dataset retains its upstream license; WinGlow's MIT does not relicense it.

The bundled dictionary provides offline GUID names and icon locations. WinGlow's own parser accepts only Text, zh-CN-Text, ResText and Icon. It resolves icons through local resources, does not execute extensions, does not import commands, and never uses downloaded names to drive automatic menu cleanup. The metadata cannot introduce writable registry paths.

The metadata is bundled with WinGlow and updates only with application releases. No independent download or daily background check runs. Existing cache files are preserved but no longer override the bundled catalog. Menu state and backups stay entirely on the user's PC.
