# Windows API reference

Primary Microsoft documentation for APIs used or proposed by the component analyses. These links describe primitives, not a claim that any one primitive reproduces POSIX semantics. Current N5 source contracts are in [process ownership](03-process-execution-job-control.md#current-n5-native-lifecycle-contract) and [control/accounting](04-signals-events-waits.md#current-n5-control-and-accounting-contract); final execution evidence belongs in the [N5 record](coverage.md#n5-implementation-and-evidence-record).

## Processes and jobs

- [`CreateProcessW`](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-createprocessw)
- [Process creation flags](https://learn.microsoft.com/en-us/windows/win32/procthread/process-creation-flags)
- [`UpdateProcThreadAttribute`](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-updateprocthreadattribute)
- [Process and thread handle inheritance](https://learn.microsoft.com/en-us/windows/win32/procthread/inheritance)
- [Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects)
- [`CreateJobObjectW`](https://learn.microsoft.com/en-us/windows/win32/api/jobapi2/nf-jobapi2-createjobobjectw)
- [`AssignProcessToJobObject`](https://learn.microsoft.com/en-us/windows/win32/api/jobapi2/nf-jobapi2-assignprocesstojobobject)
- [`SetInformationJobObject`](https://learn.microsoft.com/en-us/windows/win32/api/jobapi2/nf-jobapi2-setinformationjobobject)
- [Console process groups](https://learn.microsoft.com/en-us/windows/console/console-process-groups)
- [`GenerateConsoleCtrlEvent`](https://learn.microsoft.com/en-us/windows/console/generateconsolectrlevent)
- [`GetProcessTimes`](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getprocesstimes)
- [`GetProcessMemoryInfo`](https://learn.microsoft.com/en-us/windows/win32/api/psapi/nf-psapi-getprocessmemoryinfo)
- [`GetExitCodeProcess`](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getexitcodeprocess)
- [`QueryInformationJobObject`](https://learn.microsoft.com/en-us/windows/win32/api/jobapi2/nf-jobapi2-queryinformationjobobject)
- [`JOBOBJECT_BASIC_ACCOUNTING_INFORMATION`](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-jobobject_basic_accounting_information)
- [`JOBOBJECT_EXTENDED_LIMIT_INFORMATION`](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-jobobject_extended_limit_information) and [`JOBOBJECT_BASIC_LIMIT_INFORMATION`](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-jobobject_basic_limit_information)
- [`GetProcessIoCounters`](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getprocessiocounters)
- [`TerminateJobObject`](https://learn.microsoft.com/en-us/windows/win32/api/jobapi2/nf-jobapi2-terminatejobobject)
- [Nested jobs](https://learn.microsoft.com/en-us/windows/win32/procthread/nested-jobs)
- [When does a process ID become available for reuse?](https://devblogs.microsoft.com/oldnewthing/20110107-00/?p=11803/) — Microsoft's retained-process-object explanation.

## Console and pseudoconsole

- [`GetConsoleMode`](https://learn.microsoft.com/en-us/windows/console/getconsolemode)
- [`SetConsoleMode`](https://learn.microsoft.com/en-us/windows/console/setconsolemode)
- [Console virtual terminal sequences](https://learn.microsoft.com/en-us/windows/console/console-virtual-terminal-sequences)
- [`ReadConsoleInputW`](https://learn.microsoft.com/en-us/windows/console/readconsoleinput)
- [`GetConsoleScreenBufferInfo`](https://learn.microsoft.com/en-us/windows/console/getconsolescreenbufferinfo)
- [Pseudoconsoles](https://learn.microsoft.com/en-us/windows/console/pseudoconsoles)
- [`CreatePseudoConsole`](https://learn.microsoft.com/en-us/windows/console/createpseudoconsole)
- [`ResizePseudoConsole`](https://learn.microsoft.com/en-us/windows/console/resizepseudoconsole)

## Handles, pipes, and waits

- [`CreatePipe`](https://learn.microsoft.com/en-us/windows/win32/api/namedpipeapi/nf-namedpipeapi-createpipe)
- [`CreateNamedPipeW`](https://learn.microsoft.com/en-us/windows/win32/api/namedpipeapi/nf-namedpipeapi-createnamedpipew)
- [Synchronous and overlapped pipe I/O](https://learn.microsoft.com/en-us/windows/win32/ipc/synchronous-and-overlapped-input-and-output)
- [`DuplicateHandle`](https://learn.microsoft.com/en-us/windows/win32/api/handleapi/nf-handleapi-duplicatehandle)
- [`ReadFile`](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-readfile)
- [`WriteFile`](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-writefile)
- [`WaitForSingleObject`](https://learn.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-waitforsingleobject)
- [`WaitForMultipleObjects`](https://learn.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-waitformultipleobjects)
- [I/O completion ports](https://learn.microsoft.com/en-us/windows/win32/fileio/i-o-completion-ports)


Additional ownership, diagnostics and cancellation contracts:

- [`GetHandleInformation`](https://learn.microsoft.com/en-us/windows/win32/api/handleapi/nf-handleapi-gethandleinformation) and [`SetHandleInformation`](https://learn.microsoft.com/en-us/windows/win32/api/handleapi/nf-handleapi-sethandleinformation)
- [`CloseHandle`](https://learn.microsoft.com/en-us/windows/win32/api/handleapi/nf-handleapi-closehandle)
- [`CancelIoEx`](https://learn.microsoft.com/en-us/windows/win32/api/ioapiset/nf-ioapiset-cancelioex)
- [`FormatMessageW`](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-formatmessagew)
- [`GetLastError`](https://learn.microsoft.com/en-us/windows/win32/api/errhandlingapi/nf-errhandlingapi-getlasterror)
- [`SetConsoleCtrlHandler`](https://learn.microsoft.com/en-us/windows/console/setconsolectrlhandler)
- [`GetCurrentProcessId`](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getcurrentprocessid)
## Filesystem and notifications

- [`CreateFileW`](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew)
- [`FindFirstFileExW`](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-findfirstfileexw)
- [`GetFileInformationByHandleEx`](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getfileinformationbyhandleex)
- [`GetFinalPathNameByHandleW`](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfinalpathnamebyhandlew)
- [`CreateSymbolicLinkW`](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-createsymboliclinkw)
- [Naming files, paths, and namespaces](https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file)
- [`LockFileEx`](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-lockfileex)
- [`FlushFileBuffers`](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-flushfilebuffers)
- [`ReplaceFileW`](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-replacefilew)
- [`ReadDirectoryChangesW`](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-readdirectorychangesw)
- [`GetDriveTypeW`](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getdrivetypew)

## Identity, known folders, and locale

- [`GetUserNameExW`](https://learn.microsoft.com/en-us/windows/win32/api/secext/nf-secext-getusernameexw)
- [`SHGetKnownFolderPath`](https://learn.microsoft.com/en-us/windows/win32/api/shlobj_core/nf-shlobj_core-shgetknownfolderpath)
- [Known folders](https://learn.microsoft.com/en-us/windows/win32/shell/known-folders)
- [`GetComputerNameExW`](https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-getcomputernameexw)
- [`GetUserDefaultLocaleName`](https://learn.microsoft.com/en-us/windows/win32/api/winnls/nf-winnls-getuserdefaultlocalename)
- [`GetLocaleInfoEx`](https://learn.microsoft.com/en-us/windows/win32/api/winnls/nf-winnls-getlocaleinfoex)
- [Console code pages and Unicode guidance](https://learn.microsoft.com/en-us/windows/console/console-code-pages)

## Clipboard and shell integration

- [Clipboard operations](https://learn.microsoft.com/en-us/windows/win32/dataxchg/using-the-clipboard)
- [`ShellExecuteW`](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shellexecutew)

## Selection principles

- Use `W` APIs and UTF-16 at the OS boundary; do not use legacy ANSI `A` APIs.
- Keep handles non-inheritable by default and whitelist child inheritance with `PROC_THREAD_ATTRIBUTE_HANDLE_LIST`.
- Prefer capability checks over Windows-version checks.
- Treat Job Objects, console process groups, and Ctrl events as related primitives, not as exact POSIX process groups/signals.
- Use overlapped I/O or registered waits for scalable monitoring; do not build the design around the fixed `WaitForMultipleObjects` limit.

## Checked semantic constraints

The following contracts were checked against the linked Microsoft API documentation during this source audit:

| API contract | Consequence for fish |
|---|---|
| [`PROC_THREAD_ATTRIBUTE_HANDLE_LIST`](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-updateprocthreadattribute) requires real inheritable handles and `bInheritHandles=TRUE`; the attribute-value storage must survive until the attribute list is destroyed. | Keep ordinary handles non-inheritable, prepare an explicit child list, retain its storage and use `EXTENDED_STARTUPINFO_PRESENT`. Do not treat the list as making non-inheritable handles inheritable. |
| [`GenerateConsoleCtrlEvent`](https://learn.microsoft.com/en-us/windows/console/generateconsolectrlevent) cannot target `CTRL_C_EVENT` to a nonzero process-group ID. Only processes sharing the caller's console receive events. | `kill(SIGINT)` has no generic process-handle equivalent. Group-targeted Ctrl-Break is a different event, not interchangeable Ctrl-C behavior. A console group is not a Job Object. |
| [`SetConsoleCtrlHandler`](https://learn.microsoft.com/en-us/windows/console/setconsolectrlhandler) calls handlers on a new thread; close/logoff/shutdown behavior is bounded and conditional, and console attachment changes reset the handler table. | N5 callbacks only record epochs and signal retained events. Parser dispatch/rearm/drain is normal-thread work. Console-close is best effort, not a promise of graceful shell cleanup. No attachment changes are supported during ownership. |
| [Retaining a process HANDLE pins its process object and PID](https://devblogs.microsoft.com/oldnewthing/20110107-00/?p=11803/), including after exit. | Full `u32` native PIDs remain associated with owned live/cached/deferred-event leases; do not reopen a numerical PID and assume identity. Dropping one cache record must not invalidate another observer's process reference. |
| [`AssignProcessToJobObject`](https://learn.microsoft.com/en-us/windows/win32/api/jobapi2/nf-jobapi2-assignprocesstojobobject) restricts the **target** job's UI limits for an already-job-associated process; exceeding the target active-process limit fails assignment and terminates the specified process. Nested-job and breakaway rules also apply. | A UI-restricted enclosing host job alone does not prove assignment to a new unrestricted target will fail. Rollback still owns the newly created suspended process, including failure before membership, and must reap its retained HANDLE without touching unrelated host jobs. |
| [`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-jobobject_basic_limit_information) terminates associated processes when the last job HANDLE closes. | N5 clears this policy when a pipeline is accepted, not only at root exit/disown. Ordinary/disowned accepted work survives fish-owned job-handle closure on graceful or abrupt shell death; independent host/console policies still apply. |
| [`GetProcessTimes`](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getprocesstimes) and [Job Object user/kernel totals](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-jobobject_basic_accounting_information) use 100-nanosecond units; job totals include active and terminated associated processes. | Convert 10,000,000 ticks per second. Sum disjoint completed-cutoff plus live jobs once, never add root process CPU again. Fish's observed-root/detach cutoff intentionally excludes later descendant work; sequential snapshots are not atomic system-wide accounting. |
| [`GetProcessIoCounters`](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getprocessiocounters) measures operation/byte counts; [`GetProcessMemoryInfo`](https://learn.microsoft.com/en-us/windows/win32/api/psapi/nf-psapi-getprocessmemoryinfo) reports native memory/page-fault fields. | `--print-rusage-self` names shell-only native units and explicitly unavailable POSIX categories. Do not relabel combined faults as minor/major, I/O operations as blocks, or pagefile usage as swaps. |
| [`GetExitCodeProcess`](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getexitcodeprocess) returns a DWORD; [`TerminateJobObject`](https://learn.microsoft.com/en-us/windows/win32/api/jobapi2/nf-jobapi2-terminatejobobject) is forced termination, not a catchable signal. | Preserve every raw DWORD, map 0–255 unchanged and other native exits to shell 1. Shell cancellation 130 is separate from observed exit; a cooperative request is not acknowledgement, and unsupported/ignored Ctrl-Break never silently becomes termination. |
| [`ReplaceFileW`](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-replacefilew) requires same-volume files, preserves/merges metadata, and documents partial-failure states; `REPLACEFILE_WRITE_THROUGH` is unsupported. | Do not promise POSIX `rename` or crash durability from this call alone. Distinguish initial creation from replacement, preserve restrictive ACLs and recover according to the actual failure state. |
| [`ReadDirectoryChangesW`](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-readdirectorychangesw) watches directory contents, not the directory itself. Async operation requires an overlapped directory handle; lost/overflowed records require re-enumeration. | Watch the parent directory for atomically replaced variable/history files; treat events as invalidation hints and reopen/revalidate. Keep buffers and `OVERLAPPED` state alive through completion/cancellation. Network/filesystem support is conditional. |

These API checks validate selection and limitations, not runtime acceptance. Original documentation work did not implement or runtime-test a backend; subsequent N1–N4 evidence and N5 source overlays are recorded separately. No new gate result, artifact hash or phase-completion claim follows from an API citation.