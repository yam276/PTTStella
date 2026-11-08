//! macOS App actions enter Rust teardown, without replacing Slint's native
//! context-menu event handler or Winit's application delegate.
use crate::TerminalWindow;
use objc2::{
    DefinedClass, MainThreadOnly, define_class, msg_send, rc::Retained, runtime::Sel, sel,
};
use objc2_app_kit::{NSApplication, NSEventModifierFlags, NSMenu, NSMenuItem};
use objc2_foundation::{MainThreadMarker, NSObject, NSObjectProtocol, NSString, ns_string};
use slint::{ComponentHandle, Timer, TimerMode};
use std::{cell::Cell, rc::Rc, time::Duration};

struct TargetState {
    ui: slint::Weak<TerminalWindow>,
    stopped: Cell<bool>,
}
define_class!(
    // SAFETY: NSObject has no subclassing requirements; this type does not
    // implement Drop. AppKit invokes menu actions on its UI/main thread.
    #[unsafe(super = NSObject)]
    #[name = "PTTStellaSlintMenuTarget"]
    #[thread_kind = MainThreadOnly]
    #[ivars = TargetState]
    struct MenuTarget;
    // SAFETY: NSObjectProtocol has no additional safety requirements.
    unsafe impl NSObjectProtocol for MenuTarget {}
    impl MenuTarget {
        // SAFETY: The selector is installed only on the two NSMenuItems retained
        // below. Cocoa passes that NSMenuItem as sender, with the void action ABI.
        #[unsafe(method(activate:))]
        fn activate(&self,sender:&NSMenuItem){
            if self.ivars().stopped.get(){return;}
            if let Some(ui)=self.ivars().ui.upgrade(){
                match sender.tag(){0=>ui.invoke_app_quit(),1=>ui.invoke_settings_show(),_=>{}}
            }
        }
    }
);
impl MenuTarget {
    fn new(mtm: MainThreadMarker, ui: &TerminalWindow) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(TargetState {
            ui: ui.as_weak(),
            stopped: Cell::new(false),
        });
        // SAFETY: NSObject init has this signature; all Rust ivars are installed
        // before calling it and retained by objc2 for this object's lifetime.
        unsafe { msg_send![super(this), init] }
    }
}
pub struct Menu {
    bar: Retained<NSMenu>,
    settings: Retained<NSMenuItem>,
    quit: Retained<NSMenuItem>,
    target: Retained<MenuTarget>,
    previous: Option<Retained<NSMenu>>,
    services: Retained<NSMenu>,
    previous_services: Option<Retained<NSMenu>>,
    system_labels: Vec<(Retained<NSMenuItem>, &'static str)>,
    refresh: Timer,
    stopped: Cell<bool>,
}
impl Menu {
    pub fn mount(ui: &TerminalWindow) -> Result<Rc<Self>, String> {
        let mtm = MainThreadMarker::new().ok_or("native menu requires main thread")?;
        let app = NSApplication::sharedApplication(mtm);
        let previous = app.mainMenu();
        let previous_services = app.servicesMenu();
        let services = NSMenu::new(mtm);
        let mut system_labels = Vec::new();
        let bar = NSMenu::new(mtm);
        let entry = NSMenuItem::new(mtm);
        let submenu = NSMenu::new(mtm);
        submenu.setAutoenablesItems(false);
        submenu.setTitle(ns_string!("PTTStella Slint"));
        entry.setSubmenu(Some(&submenu));
        bar.addItem(&entry);
        let target = MenuTarget::new(mtm, ui);
        let settings = custom_item(
            mtm,
            &target,
            1,
            ",",
            &pttstella::msg!("settings-center-title"),
        );
        let quit = custom_item(mtm, &target, 0, "q", &pttstella::msg!("menu-quit"));
        let about = system_item(mtm, "", sel!(orderFrontStandardAboutPanel:), "");
        submenu.addItem(&about);
        system_labels.push((about, "slint-menu-about"));
        submenu.addItem(&NSMenuItem::separatorItem(mtm));
        submenu.addItem(&settings);
        submenu.addItem(&NSMenuItem::separatorItem(mtm));
        let services_item = NSMenuItem::new(mtm);
        services_item.setSubmenu(Some(&services));
        submenu.addItem(&services_item);
        system_labels.push((services_item, "slint-menu-services"));
        app.setServicesMenu(Some(&services));
        for (key, selector, key_equivalent) in [
            ("menu-hide", sel!(hide:), "h"),
            ("menu-hide-others", sel!(hideOtherApplications:), "h"),
            ("menu-show-all", sel!(unhideAllApplications:), ""),
        ] {
            let item = system_item(mtm, "", selector, key_equivalent);
            if key == "menu-hide-others" {
                item.setKeyEquivalentModifierMask(
                    NSEventModifierFlags::Command | NSEventModifierFlags::Option,
                );
            }
            submenu.addItem(&item);
            system_labels.push((item, key));
        }
        submenu.addItem(&NSMenuItem::separatorItem(mtm));
        submenu.addItem(&quit);
        let owner = Rc::new(Self {
            bar,
            settings,
            quit,
            target,
            previous,
            services,
            previous_services,
            system_labels,
            refresh: Timer::default(),
            stopped: Cell::new(false),
        });
        ui.on_native_menu_refresh({
            let weak = Rc::downgrade(&owner);
            move || {
                if let Some(owner) = weak.upgrade() {
                    owner.activate();
                }
            }
        });
        owner.activate();
        Ok(owner)
    }
    fn activate(self: &Rc<Self>) {
        if self.stopped.get() {
            return;
        }
        self.localize();
        let weak = Rc::downgrade(self);
        // Slint chooses a default menu during each window's Focused event. A
        // single deferred activation installs ours after that event completes.
        // No idle poll or second native context-menu handler is introduced.
        self.refresh
            .start(TimerMode::SingleShot, Duration::ZERO, move || {
                if let Some(owner) = weak.upgrade()
                    && !owner.stopped.get()
                {
                    let app = NSApplication::sharedApplication(owner.target.mtm());
                    app.setMainMenu(Some(&owner.bar));
                    app.setServicesMenu(Some(&owner.services));
                }
            });
    }
    fn localize(&self) {
        self.settings.setEnabled(
            self.target
                .ivars()
                .ui
                .upgrade()
                .is_some_and(|ui| ui.get_settings_available()),
        );
        for (item, key) in &self.system_labels {
            item.setTitle(&NSString::from_str(
                &pttstella::i18n::application().loader().get(key),
            ));
        }
        self.settings.setTitle(&NSString::from_str(&pttstella::msg!(
            "settings-center-title"
        )));
        self.quit
            .setTitle(&NSString::from_str(&pttstella::msg!("menu-quit")));
    }
    pub(crate) fn stopped(&self) -> bool {
        self.stopped.get()
            && self.target.ivars().stopped.get()
            && !self.refresh.running()
            && self.settings.action().is_none()
            && self.settings.target().is_none()
            && self.quit.action().is_none()
            && self.quit.target().is_none()
    }
    pub(crate) fn stop(&self) {
        if self.stopped.replace(true) {
            return;
        }
        self.refresh.stop();
        self.target.ivars().stopped.set(true);
        // SAFETY: Clear the weak target/action pair before our retained target
        // can be released. None removes the selector; it cannot call freed Rust.
        unsafe {
            self.settings.setAction(None);
            self.settings.setTarget(None);
            self.quit.setAction(None);
            self.quit.setTarget(None);
        }
        let app = NSApplication::sharedApplication(self.target.mtm());
        if app
            .servicesMenu()
            .is_some_and(|menu| std::ptr::eq::<NSMenu>(&*menu, &*self.services))
        {
            app.setServicesMenu(self.previous_services.as_deref());
        }

        if app
            .mainMenu()
            .is_some_and(|menu| std::ptr::eq::<NSMenu>(&*menu, &*self.bar))
        {
            app.setMainMenu(self.previous.as_deref());
        }
    }
}
impl Drop for Menu {
    fn drop(&mut self) {
        self.stop();
    }
}
fn custom_item(
    mtm: MainThreadMarker,
    target: &MenuTarget,
    tag: isize,
    key: &str,
    title: &str,
) -> Retained<NSMenuItem> {
    // SAFETY: MenuTarget implements activate: with the NSMenuItem sender ABI.
    // Menu retains both items and the target; stop clears both weak target
    // pointers before releasing it, all on the AppKit main thread.
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &NSString::from_str(title),
            Some(sel!(activate:)),
            &NSString::from_str(key),
        )
    };
    unsafe {
        item.setTarget(Some(target));
    }
    item.setTag(tag);
    item.setKeyEquivalentModifierMask(NSEventModifierFlags::Command);
    item
}
fn system_item(mtm: MainThreadMarker, title: &str, action: Sel, key: &str) -> Retained<NSMenuItem> {
    // SAFETY: All callers supply existing NSApplication actions with the void
    // sender ABI. No Rust object is installed in the system responder chain.
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &NSString::from_str(title),
            Some(action),
            &NSString::from_str(key),
        )
    };
    item.setKeyEquivalentModifierMask(NSEventModifierFlags::Command);
    item
}
