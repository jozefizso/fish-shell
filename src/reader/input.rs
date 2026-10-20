//! Reader implementation of InputEventQueuer.
use super::{Reader, reader_reading_interrupted, reader_schedule_prompt_repaint};
use crate::{
    event,
    input::{CharEvent, InputData, InputEventQueuer, ReadlineCmd, input_get_bind_mode},
    proc::job_reap,
    signal::signal_clear_cancel,
};
use fish_common::escape;
use fish_widestring::{WString, bytes2wcstring};
#[cfg(unix)]
use std::os::fd::RawFd;

impl<'a> InputEventQueuer for Reader<'a> {
    fn get_input_data(&self) -> &InputData {
        &self.input_data
    }

    fn get_input_data_mut(&mut self) -> &mut InputData {
        &mut self.input_data
    }

    #[cfg(unix)]
    fn get_ioport_fd(&self) -> RawFd {
        self.debouncers.event_signaller_read_fd()
    }

    fn prepare_to_select(&mut self) {
        #[cfg(windows)]
        let evaluation = crate::console_windows::begin_evaluation_input();
        // Fire any pending events and reap stray processes, including printing exit status messages.
        event::fire_delayed(self.parser);
        if job_reap(self.parser, true, None) {
            #[cfg(windows)]
            crate::screen::invalidate_native_screen();
            reader_schedule_prompt_repaint();
        }
        #[cfg(windows)]
        self.reconcile_evaluation_input(&evaluation);
        #[cfg(windows)]
        if self.parser.libdata().exit_current_script {
            self.input_data.native_exit_status = Some(self.parser.last_status());
            self.exit_loop_requested = true;
            self.push_front(CharEvent::from_check_exit());
        }
    }

    #[cfg(unix)]
    fn select_interrupted(&mut self) {
        // Readline commands may be bound to \cc which also sets the cancel flag.
        // See #6937, #8125.
        signal_clear_cancel();

        // Fire any pending events and reap stray processes, including printing exit status messages.
        let parser = &mut *self.parser;
        event::fire_delayed(parser);
        if job_reap(parser, true, None) {
            reader_schedule_prompt_repaint();
        }

        // Tell the reader an event occurred.
        if reader_reading_interrupted(self) != 0 {
            self.enqueue_interrupt_key();
            return;
        }
        self.push_front(CharEvent::from_check_exit());
    }

    #[cfg(windows)]
    fn wait_input_event(
        &mut self,
        timeout: crate::fd_readable_set::Timeout,
    ) -> crate::input::InputEventTrigger {
        let jobs = crate::proc::native_runtime_if_initialized(self.parser);
        let state = self.parser.variables.native_state_runtime();
        let data = &mut *self.data;
        data.input_data.native.next(
            timeout,
            crate::input::WaitSources {
                workers: Some(data.debouncers.runtime.notification_handle()),
                state: state.as_ref().map(|runtime| runtime.notification_handle()),
                jobs: jobs.as_ref().map(|runtime| runtime.notification_handle()),
            },
        )
    }

    #[cfg(windows)]
    fn input_error(&mut self, error: std::io::Error) {
        self.input_data.native_error = Some(error);
        self.exit_loop_requested = true;
        // Notification callbacks must wake character-only binding waits too.
        self.push_front(CharEvent::from_check_exit());
    }
    #[cfg(windows)]
    fn select_interrupted(&mut self) {
        let controls = crate::signal::pending_console_controls();
        if let Err(error) = self.input_data.native.interrupt() {
            self.input_error(error);
            return;
        }
        self.input_data.queue.clear();
        let had_paste = self.input_data.paste_buffer.take().is_some();
        let cancelled = controls.ctrl_c != 0 || controls.ctrl_break != 0;
        self.input_data.native_cancelled |= cancelled;
        if cancelled {
            crate::reader::native_reader_interrupt();
        }
        // Keep the actual receipt above. Rearm never consumes counted event
        // delivery; it lets their real handlers evaluate with a fresh token.
        if let Err(error) = crate::signal::rearm_console_controls() {
            self.input_error(error);
            return;
        }
        event::fire_delayed(self.parser);
        self.native_jobs_notified();
        if self.parser.libdata().exit_current_script {
            self.input_data.native_exit_status = Some(self.parser.last_status());
            self.exit_loop_requested = true;
        }
        if controls.close != 0 {
            crate::reader::native_console_closed();
        }
        if !self.exit_loop_requested && controls.close == 0 {
            if let Err(error) = self.input_data.native.resume() {
                self.input_error(error);
                return;
            }
        }
        if !self.exit_loop_requested && reader_reading_interrupted(self) != 0 {
            self.enqueue_interrupt_key();
        } else {
            self.push_front(CharEvent::from_check_exit());
        }
        if had_paste {
            self.push_front(CharEvent::from_readline(ReadlineCmd::EndUndoGroup, vec![]));
        }
    }

    #[cfg(windows)]
    fn native_jobs_notified(&mut self) {
        let evaluation = crate::console_windows::begin_evaluation_input();
        if job_reap(self.parser, true, None) {
            crate::screen::invalidate_native_screen();
            reader_schedule_prompt_repaint();
        }
        self.reconcile_evaluation_input(&evaluation);
        if self.parser.libdata().exit_current_script {
            self.input_data.native_exit_status = Some(self.parser.last_status());
            self.exit_loop_requested = true;
            self.push_front(CharEvent::from_check_exit());
        }
    }

    fn uvar_change_notified(&mut self) {
        #[cfg(windows)]
        let evaluation = crate::console_windows::begin_evaluation_input();
        #[cfg(unix)]
        self.parser.sync_uvars_and_fire(true /* always */);
        #[cfg(windows)]
        match self.parser.variables.service_native_state() {
            Ok(events) => {
                for event in events {
                    event::fire(self.parser, event);
                }
                if self.parser.libdata().exit_current_script {
                    self.input_data.native_exit_status = Some(self.parser.last_status());
                    self.exit_loop_requested = true;
                    self.push_front(CharEvent::from_check_exit());
                }
            }
            Err(error) => self.input_error(error),
        };
        #[cfg(windows)]
        self.reconcile_evaluation_input(&evaluation);
    }

    fn ioport_notified(&mut self) {
        // Our iothread signaller was posted, indicating some debouncer has a new result.
        #[cfg(unix)]
        self.debouncers.event_signaller.try_consume();
        self.service_debounced_results();
    }

    fn paste_start_buffering(&mut self) {
        self.input_data.paste_buffer = Some(vec![]);
        self.push_front(CharEvent::from_readline(
            ReadlineCmd::BeginUndoGroup,
            vec![],
        ));
    }

    fn paste_commit(&mut self) {
        self.push_front(CharEvent::from_readline(ReadlineCmd::EndUndoGroup, vec![]));
        let Some(buffer) = self.input_data.paste_buffer.take() else {
            return;
        };
        self.push_front(CharEvent::Command(sprintf!(
            "__fish_paste %s",
            escape(&bytes2wcstring(&buffer))
        )));
    }

    fn get_bind_mode(&self) -> WString {
        input_get_bind_mode(self.parser.vars())
    }
}
