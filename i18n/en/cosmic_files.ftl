cosmic-files = COSMIC Files
comment = File manager for the COSMIC desktop
keywords = Folder;Manager;
empty-folder = Empty folder
empty-folder-hidden = Empty folder (has hidden items)
no-results = No results found
filesystem = Filesystem
home = Home
networks = Networks
notification-in-progress = File operations are in progress
trash = Trash
recents = Recents
undo = Undo
today = Today

# Desktop view options
desktop-view-options = Desktop view options...
show-on-desktop = Show on Desktop
desktop-folder-content = Desktop folder content
mounted-drives = Mounted drives
trash-folder-icon = Trash folder icon
icon-size-and-spacing = Icon size and spacing
icon-size = Icon size
grid-spacing = Grid spacing

# List view
name = Name
modified = Modified
trashed-on = Trashed
size = Size

# Progress footer
details = Details
dismiss = Dismiss message
operations-running = {$running} {$running ->
    [one] operation
    *[other] operations
  } running ({$percent}%)...
operations-running-finished = {$running} {$running ->
    [one] operation
    *[other] operations
  } running ({$percent}%), {$finished} finished...
pause = Pause
resume = Resume

# Dialogs

## Compress Dialog
create-archive = Create archive

## Copy To Dialog
copy-to-title = Select copy destination
copy-to-button-label = Copy

## Extract Dialog
extract-password-required = Password required
extract-to = Extract To...
extract-to-title = Extract to folder

## Empty Trash Dialog
empty-trash = Empty trash
empty-trash-title = Empty trash?
empty-trash-warning = Items in the Trash folder will be permanently deleted

## Mount Error Dialog
mount-error = Unable to access drive

## Move To Dialog
move-to-title = Select move destination
move-to-button-label = Move

## New File/Folder Dialog
create-new-file = Create new file
create-new-folder = Create new folder
file-name = File name
folder-name = Folder name
file-already-exists = A file with that name already exists
folder-already-exists = A folder with that name already exists
name-hidden = Names starting with "." will be hidden
name-invalid = Name cannot be "{$filename}"
name-no-slashes = Name cannot contain slashes

## Open/Save Dialog
cancel = Cancel
create = Create
open = Open
open-file = Open file
open-folder = Open folder
open-in-new-tab = Open in new tab
open-in-new-window = Open in new window
open-item-location = Open item location
find-similar = Find similar files
organize = Organize
organize-planning = Planning {$n} {$n ->
    [one] file
   *[other] files
}
organize-planning-start = Planning…
organize-waited = {$s} s so far
organize-progress = {$done} of {$total} planned, {$s} s so far
organize-new = New
organize-move = Move {$n}
organize-every-file = Show every file
organize-summary = {$total} {$total ->
    [one] file
   *[other] files
}: {$moving} move into {$folders} {$folders ->
    [one] folder
   *[other] folders
} ({$new} new), {$asking} to ask about, {$staying} {$staying ->
    [one] stays
   *[other] stay
}
organize-summary-none = {$total} {$total ->
    [one] file
   *[other] files
}: nothing ticked to move, {$asking} to ask about, {$staying} {$staying ->
    [one] stays
   *[other] stay
}
organize-all-from = all from {$folder}
organize-all-from-flat = all from {$folder}, subfolders not included
organize-no-subfolders = Subfolders not included
organize-tab-move = Move
organize-tab-ask = Ask
organize-tab-stay = Stay
organize-legend-move = Move {$n}
organize-legend-ask = Ask {$n}
organize-legend-stay = Stay {$n}
organize-staying = Staying
organize-staying-in = Staying in {$folder}
organize-folders = {$n} {$n ->
    [one] folder
   *[other] folders
}
organize-filter = Filter
organize-send-to = Send to
organize-send-title = Send to
organize-send-browse = Browse…
organize-send-label = Send here
organize-trash-copy = Trash this copy
organize-no-match = No files match
organize-sent-note = you picked this folder
organize-why-yours = Where you sent them
organize-name-label = Name
organize-name-empty = Give the new folder a name
organize-name-slash = A folder name can't hold a /
organize-name-dots = A folder can't be named . or ..
organize-name-hidden = A name starting with a dot hides the folder
organize-name-bad = Not a name the disk takes: too long, or a NUL in it
organize-discard-title = Discard changes?
organize-discard-body = The ticks, names and folders you changed in this plan will be lost.
organize-discard = Discard
organize-keep-editing = Keep editing
organize-to = To
organize-from = From
organize-in = In
organize-files = {$n ->
    [one] file
   *[other] files
}
organize-photos = {$n ->
    [one] photo
   *[other] photos
}
organize-say-make = Make a new folder {$folder} and move {$n} {$noun}
organize-say-move = Move {$n} {$noun} into {$folder}
organize-say-maybe-make = Maybe make a new folder {$folder} and move {$n} {$noun}
organize-say-maybe-move = Maybe move {$n} {$noun} into {$folder}
organize-say-join = Move {$n} {$noun} into the existing folder {$folder}
organize-say-maybe-join = Maybe move {$n} {$noun} into the existing folder {$folder}
organize-say-unticked = {$said}; {$n} more unticked
organize-leave = {$n ->
    [one] Leave 1 file where it is: {$why}
   *[other] Leave {$n} files where they are: {$why}
}
organize-more = +{$n} more
organize-fewer = Fewer
organize-col-file = File
organize-col-to = Goes to
organize-col-why = Why
organize-n-move = {$n} move
organize-n-ask = {$n} ask
organize-n-stay = {$n} stay
organize-stays = stays here
organize-moved-one = Moved {$n} {$n ->
    [one] file
   *[other] files
} into {$folder}
organize-moved = Moved {$n} {$n ->
    [one] file
   *[other] files
} into {$folders} folders, most into {$top}
organize-moved-left = {$said}; {$left} left where {$left ->
    [one] it was
   *[other] they were
}
organize-status = organized {$n} {$n ->
    [one] file
   *[other] files
} into {$folders} {$folders ->
    [one] folder
   *[other] folders
}
organize-and = {$a} and {$b}
organize-trashed = {$said}; {$n} {$n ->
    [one] copy
   *[other] copies
} to the trash
organize-summary-trash = {$said}; {$n} {$n ->
    [one] copy
   *[other] copies
} to trash
organize-keep-copy = Keep this copy
organize-will-trash = will be trashed
organize-unrename = Undo rename
organize-carry = Also move {$from} to {$to}?
organize-carry-more = Also move {$from} to {$to}, and {$n} more {$n ->
    [one] folder
   *[other] folders
} inside?
organize-carry-yes = Yes
organize-carry-no = No, keep it
organize-gone = The folder being organized is gone; the plan was dropped
undo-put-back = Put back {$n} {$n ->
    [one] file
   *[other] files
}
undo-nothing = Nothing to undo
undo-already = Already put back
undo-failed = Undo failed: is ariadne running?
organize-nothing = Nothing moved
organize-failed = Organize failed: is ariadne running?
folder-summary = Summary
inbox = Inbox
inbox-empty = Nothing new
inbox-move = Move
inbox-keep = Keep here
inbox-undo = Undo
search-placeholder = Search by name or by meaning
tidy = Tidy
tidy-all = Do all
tidy-skip = Skip
tidy-empty = Nothing to tidy
tidy-close = Close
palette = Palette
palette-file = Palette file
font = Font
icons = Icons
motion = Motion
sound = Sound
open-multiple-files = Open multiple files
open-multiple-folders = Open multiple folders
save = Save
save-file = Save file

## Open With Dialog
open-with-title = How do you want to open "{$name}"?
browse-store = Browse {$store}
other-apps = Other applications
related-apps = Related applications
context-action = Context action
context-action-confirm-title = Run "{$name}"?
context-action-confirm-warning = This will run on {$items} {$items ->
    [one] item
    *[other] items
  }.
run = Run

## Permanently delete Dialog
selected-items = The {$items} selected items
permanently-delete-question = Permanently delete?
delete = Delete
permanently-delete-warning = {$target} will be permanently deleted. This action can't be undone.

## Rename Dialog
rename-file = Rename file
rename-folder = Rename folder
rename-confirm = Rename

## Change Sidebar Label Dialog
sidebar-label = Sidebar label

## Replace Dialog
replace = Replace
replace-title = "{$filename}" already exists in this location
replace-warning = Do you want to replace it with the one you are saving? Replacing it will overwrite its content.
replace-warning-operation = Do you want to replace it? Replacing it will overwrite its content.
original-file = Original file
replace-with = Replace with
apply-to-all = Apply to all
keep-both = Keep both
skip = Skip

## Set as Executable and Launch Dialog
set-executable-and-launch = Set as executable and launch
set-executable-and-launch-description = Do you want to set "{$name}" as executable and launch it?
set-and-launch = Set and launch

## Metadata Dialog
open-with = Open with
owner = Owner
group = Group
other = Other
mixed = Mixed
### Mode 0
none = None
### Mode 1 (unusual)
execute-only = Execute-only
### Mode 2 (unusual)
write-only = Write-only
### Mode 3 (unusual)
write-execute = Write and execute
### Mode 4
read-only = Read-only
### Mode 5
read-execute = Read and execute
### Mode 6
read-write = Read and write
### Mode 7
read-write-execute = Read, write, and execute

## Favorite Path Error Dialog
favorite-path-error = Error opening directory
favorite-path-error-description =
    Unable to open "{$path}"
    "{$path}" might not exist or you might not have permission to open it

    Would you like to remove it from the sidebar?
remove = Remove
keep = Keep

# Context Pages

## About
repository = Repository
support = Support

## Add Network Drive
add-network-drive = Add network drive
connect = Connect
connect-anonymously = Connect anonymously
connecting = Connecting...
domain = Domain
enter-server-address = Enter server address
network-drive-description =
    Server addresses include a protocol prefix and address.
    Examples: ssh://192.168.0.1, ftp://[2001:db8::1]
### Make sure to keep the comma which separates the columns
network-drive-schemes =
    Available protocols,Prefix
    AppleTalk,afp://
    File Transfer Protocol,ftp:// or ftps://
    Network File System,nfs://
    Server Message Block,smb://
    SSH File Transfer Protocol,sftp:// or ssh://
    WebDAV,dav:// or davs://
network-drive-error = Unable to access network drive
password = Password
remember-password = Remember password
try-again = Try again
username = Username

## Operations
cancelled = Cancelled
edit-history = Edit history
history = History...
no-history = No items in history.
pending = Pending
progress = {$percent}%
progress-cancelled = {$percent}%, cancelled
progress-failed = {$percent}%, failed
progress-paused = {$percent}%, paused
failed = Failed
complete = Complete
compressing = Compressing {$items} {$items ->
        [one] item
        *[other] items
    } from "{$from}" to "{$to}" ({$progress})...
compressed = Compressed {$items} {$items ->
        [one] item
        *[other] items
    } from "{$from}" to "{$to}"
copy_noun = Copy
pasted-image = Pasted Image
pasted-text = Pasted Text
pasted-video = Pasted Video
creating = Creating "{$name}" in "{$parent}"
created = Created "{$name}" in "{$parent}"
copying = Copying {$items} {$items ->
        [one] item
        *[other] items
    } from "{$from}" to "{$to}" ({$progress})...
copied = Copied {$items} {$items ->
        [one] item
        *[other] items
    } from "{$from}" to "{$to}"
deleting = Deleting {$items} {$items ->
        [one] item
        *[other] items
    } from {trash} ({$progress})...
deleted = Deleted {$items} {$items ->
        [one] item
        *[other] items
    } from {trash}
emptying-trash = Emptying {trash} ({$progress})...
emptied-trash = Emptied {trash}
extracting = Extracting {$items} {$items ->
        [one] item
        *[other] items
    } from "{$from}" to "{$to}" ({$progress})...
extracted = Extracted {$items} {$items ->
        [one] item
        *[other] items
    } from "{$from}" to "{$to}"
setting-executable-and-launching = Setting "{$name}" as executable and launching
set-executable-and-launched = Set "{$name}" as executable and launched
setting-permissions = Setting permissions for "{$name}" to {$mode}
set-permissions = Set permissions for "{$name}" to {$mode}
moving = Moving {$items} {$items ->
        [one] item
        *[other] items
    } from "{$from}" to "{$to}" ({$progress})...
moved = Moved {$items} {$items ->
        [one] item
        *[other] items
    } from "{$from}" to "{$to}"
permanently-deleting = Permanently deleting {$items} {$items ->
        [one] item
        *[other] items
    }
permanently-deleted = Permanently deleted {$items} {$items ->
        [one] item
        *[other] items
    }
removing-from-recents = Removing {$items} {$items ->
        [one] item
        *[other] items
    } from {recents}
removed-from-recents = Removed {$items} {$items ->
        [one] item
        *[other] items
    } from {recents}
renaming = Renaming "{$from}" to "{$to}"
renamed = Renamed "{$from}" to "{$to}"
restoring = Restoring {$items} {$items ->
        [one] item
        *[other] items
    } from {trash} ({$progress})...
restored = Restored {$items} {$items ->
        [one] item
        *[other] items
    } from {trash}
unknown-folder = unknown folder

## Open with
menu-open-with = Open with...
default-app = {$name} (default)
search-application = Search by app name

## Show details
show-details = Show details...
type = Type: {$mime}
items = Items: {$items}
item-size = Size: {$size}
item-created = Created: {$created}
item-modified = Modified: {$modified}
item-accessed = Accessed: {$accessed}
calculating = Calculating...
checksum = {$kind} checksum
calculate = Calculate
error = Error

## Settings
settings = Settings
single-click = Single click to open
show-recents = Recents folder in the sidebar

### Appearance
appearance = Appearance
theme = Theme
match-desktop = Match desktop
dark = Dark
light = Light

### Type to search
type-to-search = Type to search
type-to-search-recursive = Searches the current folder and all subfolders
type-to-search-enter-path = Enters the path to the directory or file
type-to-search-select = Selects the first matching file or folder

# Context menu
add-to-sidebar = Add to sidebar
clear-recents-history = Clear Recents history
compress = Compress...
copy-to = Copy to...
delete-permanently = Delete permanently
eject = Eject
extract-here = Extract
new-file = New file...
new-folder = New folder...
open-in-terminal = Open in terminal
move-to = Move to...
move-to-trash = Move to trash
restore-from-trash = Restore from trash
remove-from-sidebar = Remove from sidebar
change-sidebar-label = Change sidebar label
sort-by-name = Sort by name
sort-by-modified = Sort by modified
sort-by-size = Sort by size
sort-by-trashed = Sort by delete time
remove-from-recents = Remove from recents

## Desktop
change-wallpaper = Change wallpaper...
desktop-appearance = Desktop appearance...
display-settings = Display settings...

# Menu

## File
file = File
new-tab = New tab
new-window = New window
reload-folder = Reload folder
rename = Rename...
close-tab = Close tab
quit = Quit

## Edit
edit = Edit
cut = Cut
copy = Copy
copy-path = Copy path
paste = Paste
select-all = Select all

## View
zoom-in = Zoom in
default-size = Default size
zoom-out = Zoom out
view = View
grid-view = Grid view
map-view = Map view
list-view = List view
view-grid = Grid
view-list = List
view-map = Map
show-hidden-files = Show hidden files
list-directories-first = List directories first
gallery-preview = Gallery preview
menu-settings = Settings...
menu-about = About COSMIC Files...

## Sort
sort = Sort
sort-a-z = A-Z
sort-z-a = Z-A
sort-newest-first = Newest first
sort-oldest-first = Oldest first
sort-smallest-to-largest = Smallest to largest
sort-largest-to-smallest = Largest to smallest
