//! SDL2 owns only the desktop window and events. Vulkan renders the pixels;
//! presentation uses a software surface blit, never an SDL/OpenGL renderer.
use super::*;

pub(super) const SDL_FUNCTIONS: &[(&str, u32)] = &[
    ("SDL_InitSubSystem", 2048),
    ("SDL_QuitSubSystem", 2056),
    ("SDL_CreateWindow", 2064),
    ("SDL_DestroyWindow", 2072),
    ("SDL_GetWindowSurface", 2080),
    ("SDL_CreateRGBSurfaceFrom", 2088),
    ("SDL_UpperBlitScaled", 2096),
    ("SDL_UpdateWindowSurface", 2104),
    ("SDL_FreeSurface", 2112),
    ("SDL_PollEvent", 2120),
    ("SDL_GetWindowSize", 2128),
    ("SDL_GetTicks64", 2136),
    ("SDL_GetWindowID", 2144),
    ("SDL_SetSurfaceBlendMode", 2152),
];
const SDL_WINDOW: u32 = 4096;
const SDL_START_TICKS: u32 = 4112;
const SDL_WINDOW_ID: u32 = 4120;
const SDL_INITIALIZED: u32 = 4128;
const SDL_LOADED: u32 = 4136;
const EVENT: u32 = 16384;
const TITLE: u32 = 16512;

impl Code<'_> {
    pub(super) fn vk_window_runtime(&mut self) {
        self.vk_sdl_open();
        self.vk_sdl_poll();
        self.vk_sdl_present();
        self.vk_sdl_close();
    }

    fn vk_event_u32(&mut self, offset: u32, reg: u32) {
        self.vk_addr(EVENT + offset, reg);
        self.emit(0xb9400000 | reg << 5 | reg);
    }
    fn vk_event_byte(&mut self, offset: u32, reg: u32) {
        self.vk_addr(EVENT + offset, reg);
        self.ldrb(reg, reg);
    }
    fn vk_sdl_open(&mut self) {
        self.mark(self.vulkan.window_open);
        self.prologue(FRAME);
        self.store(0, 0);
        self.store(1, 1);
        self.store(2, 2);
        let loaded = self.label();
        let failed = self.label();
        let success = self.label();
        self.vk_get(SDL_LOADED, 0);
        self.branch(loaded, Branch::Nonzero(0));
        self.vk_load_functions("libSDL2-2.0.so.0", SDL_FUNCTIONS, failed);
        self.vk_imm(SDL_LOADED, 1);
        self.mark(loaded);
        self.vk_call("SDL_InitSubSystem", &[Arg::Imm(0x20)]);
        self.branch(failed, Branch::Nonzero(0));
        self.vk_imm(SDL_INITIALIZED, 1);

        // Dunder strings are length-prefixed, not NUL terminated. Reject long
        // titles and embedded NUL instead of silently truncating them.
        self.load(0);
        self.ldr(1, 0);
        self.imm(2, 4095);
        self.cmp(1, 2);
        self.branch(failed, Branch::Cond(8));
        self.add_imm(0, 0, 8);
        self.vk_addr(TITLE, 2);
        let copy = self.label();
        let copied = self.label();
        self.mark(copy);
        self.branch(copied, Branch::Zero(1));
        self.ldrb(3, 0);
        self.branch(failed, Branch::Zero(3));
        self.strb(3, 2);
        self.add_imm(0, 0, 1);
        self.add_imm(2, 2, 1);
        self.sub_imm(1, 1, 1);
        self.branch(copy, Branch::Jump);
        self.mark(copied);
        self.strb(31, 2);
        self.vk_call(
            "SDL_CreateWindow",
            &[
                Arg::Addr(TITLE),
                Arg::Imm(0x2fff0000),
                Arg::Imm(0x2fff0000),
                Arg::Slot(1),
                Arg::Slot(2),
                Arg::Imm(0x24), // SDL_WINDOW_SHOWN | SDL_WINDOW_RESIZABLE
            ],
        );
        self.vk_set(SDL_WINDOW, 0);
        self.branch(failed, Branch::Zero(0));
        self.vk_call("SDL_GetWindowID", &[Arg::Ctx(SDL_WINDOW)]);
        self.vk_set(SDL_WINDOW_ID, 0);
        self.vk_call("SDL_GetTicks64", &[]);
        self.vk_set(SDL_START_TICKS, 0);
        self.vk_imm(TIME, 0);
        self.load(1);
        self.vk_set(WINDOW_WIDTH, 0);
        self.load(2);
        self.vk_set(WINDOW_HEIGHT, 0);
        self.vk_imm(CLOSED, 0);
        self.vk_imm(POINTER_DOWN, 0);
        self.vk_imm(POINTER_X, 0);
        self.vk_imm(POINTER_Y, 0);
        self.vk_imm(KEY, u64::MAX);
        self.vk_imm(RESIZED, 1);
        self.branch(success, Branch::Jump);
        self.mark(failed);
        self.branch(self.vulkan.window_close, Branch::Call);
        self.imm(0, 0);
        self.epilogue();
        self.mark(success);
        self.imm(0, 1);
        self.epilogue();
    }

    fn vk_sdl_poll(&mut self) {
        self.mark(self.vulkan.window_poll);
        self.prologue(FRAME);
        let done = self.label();
        let events = self.label();
        let next = self.label();
        let finish = self.label();
        let close = self.label();
        let window_event = self.label();
        let keyboard = self.label();
        let motion = self.label();
        let button = self.label();
        self.vk_imm(KEY, u64::MAX);
        self.vk_imm(RESIZED, 0);
        self.vk_get(SDL_WINDOW, 0);
        self.branch(done, Branch::Zero(0));
        self.vk_get(CLOSED, 0);
        self.branch(finish, Branch::Nonzero(0));
        self.imm(0, 256);
        self.store(0, 0);
        self.mark(events);
        self.vk_call("SDL_PollEvent", &[Arg::Addr(EVENT)]);
        self.branch(finish, Branch::Zero(0));
        self.vk_event_u32(0, 0);
        self.imm(1, 0x100); // SDL_QUIT
        self.cmp(0, 1);
        self.branch(close, Branch::Cond(0));
        // Window IDs prevent queued events from a closed generation leaking
        // into a newly opened window.
        self.store(1, 0);
        self.vk_event_u32(8, 0);
        self.vk_get(SDL_WINDOW_ID, 1);
        self.cmp(0, 1);
        self.branch(next, Branch::Cond(1));
        self.load(1);
        for (kind, label) in [
            (0x200, window_event),
            (0x300, keyboard),
            (0x400, motion),
            (0x401, button),
            (0x402, button),
        ] {
            self.imm(1, kind);
            self.cmp(0, 1);
            self.branch(label, Branch::Cond(0));
        }
        self.branch(next, Branch::Jump);

        self.mark(window_event);
        self.vk_event_byte(12, 0);
        self.imm(1, 14); // SDL_WINDOWEVENT_CLOSE
        self.cmp(0, 1);
        self.branch(close, Branch::Cond(0));
        let resize = self.label();
        for kind in [5, 6] {
            self.imm(1, kind);
            self.cmp(0, 1);
            self.branch(resize, Branch::Cond(0));
        }
        let focus = self.label();
        self.imm(1, 13); // SDL_WINDOWEVENT_FOCUS_LOST
        self.cmp(0, 1);
        self.branch(focus, Branch::Cond(0));
        self.branch(next, Branch::Jump);
        self.mark(focus);
        self.vk_imm(POINTER_DOWN, 0);
        self.branch(finish, Branch::Jump);
        self.mark(resize);
        self.vk_imm(RESIZED, 1);
        self.branch(finish, Branch::Jump);

        self.mark(keyboard);
        self.vk_event_u32(20, 0); // SDL_Keysym.sym: printable ASCII or SDL keycode
        self.emit(0x93407c00); // sxtw x0,w0
        self.vk_set(KEY, 0);
        self.branch(finish, Branch::Jump);

        self.mark(motion);
        self.vk_event_u32(20, 0);
        self.emit(0x93407c00);
        self.vk_set(POINTER_X, 0);
        self.vk_event_u32(24, 0);
        self.emit(0x93407c00);
        self.vk_set(POINTER_Y, 0);
        self.vk_event_u32(16, 0);
        self.imm(1, 1);
        self.emit(0x8a010000); // and x0,x0,x1: left button mask
        self.vk_set(POINTER_DOWN, 0);
        self.branch(finish, Branch::Jump);

        self.mark(button);
        self.vk_event_byte(16, 0);
        self.imm(1, 1); // SDL_BUTTON_LEFT
        self.cmp(0, 1);
        self.branch(next, Branch::Cond(1));
        self.vk_event_u32(20, 0);
        self.emit(0x93407c00);
        self.vk_set(POINTER_X, 0);
        self.vk_event_u32(24, 0);
        self.emit(0x93407c00);
        self.vk_set(POINTER_Y, 0);
        self.vk_event_byte(17, 0);
        self.vk_set(POINTER_DOWN, 0);
        self.branch(finish, Branch::Jump);

        self.mark(close);
        self.vk_imm(CLOSED, 1);
        self.vk_imm(POINTER_DOWN, 0);
        self.branch(finish, Branch::Jump);
        self.mark(next);
        self.load(0);
        self.sub_imm(0, 0, 1);
        self.store(0, 0);
        self.branch(events, Branch::Nonzero(0));
        self.mark(finish);
        self.vk_call(
            "SDL_GetWindowSize",
            &[
                Arg::Ctx(SDL_WINDOW),
                Arg::Addr(WINDOW_WIDTH),
                Arg::Addr(WINDOW_HEIGHT),
            ],
        );
        self.vk_call("SDL_GetTicks64", &[]);
        self.vk_get(SDL_START_TICKS, 1);
        self.emit(0xcb010000); // sub x0,x0,x1
        self.vk_set(TIME, 0);
        self.mark(done);
        self.imm(0, 1);
        self.epilogue();
    }

    fn vk_sdl_present(&mut self) {
        self.mark(self.vulkan.window_present);
        self.prologue(FRAME);
        let done = self.label();
        let failed = self.label();
        let free_failed = self.label();
        self.vk_get(SDL_WINDOW, 0);
        self.branch(done, Branch::Zero(0));
        self.vk_get(CLOSED, 0);
        self.branch(failed, Branch::Nonzero(0));
        self.vk_call("SDL_GetWindowSurface", &[Arg::Ctx(SDL_WINDOW)]);
        self.store(0, 0);
        self.branch(failed, Branch::Zero(0));
        self.vk_get(WIDTH, 0);
        self.emit(0xd37ef400); // lsl x0,x0,#2: RGBA row pitch
        self.store(1, 0);
        self.vk_call(
            "SDL_CreateRGBSurfaceFrom",
            &[
                Arg::Ctx(PIXELS),
                Arg::Ctx(WIDTH),
                Arg::Ctx(HEIGHT),
                Arg::Imm(32),
                Arg::Slot(1),
                Arg::Imm(0xff),
                Arg::Imm(0xff00),
                Arg::Imm(0xff0000),
                Arg::Imm(0xff000000),
            ],
        );
        self.store(2, 0);
        self.branch(failed, Branch::Zero(0));
        // The Vulkan render pass has already composited alpha. Copy its pixels
        // directly instead of blending the finished image a second time.
        self.vk_call("SDL_SetSurfaceBlendMode", &[Arg::Slot(2), Arg::Imm(0)]);
        self.branch(free_failed, Branch::Nonzero(0));
        self.vk_call(
            "SDL_UpperBlitScaled",
            &[Arg::Slot(2), Arg::Imm(0), Arg::Slot(0), Arg::Imm(0)],
        );
        self.branch(free_failed, Branch::Nonzero(0));
        self.vk_call("SDL_FreeSurface", &[Arg::Slot(2)]);
        self.vk_call("SDL_UpdateWindowSurface", &[Arg::Ctx(SDL_WINDOW)]);
        self.branch(failed, Branch::Nonzero(0));
        self.branch(done, Branch::Jump);
        self.mark(free_failed);
        self.vk_call("SDL_FreeSurface", &[Arg::Slot(2)]);
        self.mark(failed);
        self.imm(0, 0);
        self.epilogue();
        self.mark(done);
        self.imm(0, 1);
        self.epilogue();
    }

    fn vk_sdl_close(&mut self) {
        self.mark(self.vulkan.window_close);
        self.prologue(FRAME);
        let no_window = self.label();
        let done = self.label();
        self.vk_get(SDL_WINDOW, 0);
        self.branch(no_window, Branch::Zero(0));
        self.vk_call("SDL_DestroyWindow", &[Arg::Ctx(SDL_WINDOW)]);
        self.vk_imm(SDL_WINDOW, 0);
        self.vk_imm(SDL_WINDOW_ID, 0);
        self.mark(no_window);
        self.vk_get(SDL_INITIALIZED, 0);
        self.branch(done, Branch::Zero(0));
        self.vk_call("SDL_QuitSubSystem", &[Arg::Imm(0x20)]);
        self.vk_imm(SDL_INITIALIZED, 0);
        self.mark(done);
        self.imm(0, 0);
        self.epilogue();
    }
}
