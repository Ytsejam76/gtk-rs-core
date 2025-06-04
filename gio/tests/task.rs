#![allow(unused)]
use std::ffi::c_void;
use crate::ffi::GTask;
use gio::*;
use glib::translate::*;
use glib::{gobject_ffi, MainContext, MainLoop};
use std::sync::mpsc::{channel, Sender};

pub fn run_async_local<T: 'static, Q: FnOnce(Sender<T>, MainLoop) + Send + 'static>(start: Q) -> T {
    let c = MainContext::new();
    let l = MainLoop::new(Some(&c), false);
    let l_clone = l.clone();

    let (tx, rx) = channel();

    c.spawn_local(async move {
        start(tx, l_clone);
    });

    l.run();

    rx.recv().unwrap()
}

fn run_task<F: FnOnce(&LocalTask<bool>, &glib::Object) + Send + 'static>(
    f: F,
) -> Result<bool, glib::Error> {
    run_async_local(|tx, l| {
        let cancellable = crate::Cancellable::new();

        let obj: glib::Object = glib::Object::new();
        let task = unsafe {
            crate::LocalTask::<bool>::new(
                Some(&obj),
                Some(&cancellable),
                move |t: LocalTask<bool>, _b: Option<&glib::Object>| {
                    let res = t.propagate();

                    eprintln!("Got result: {:?}", res);
                    tx.send(res).unwrap();
                    l.quit();
                },
            )
        };

        // This works
        // task.return_result(Ok(true));
        f(&task, &obj);
    })
}

fn main() {}

#[test]
fn test_return_boolean() {
    assert_eq!(
        Ok(true),
        run_task(|t: &LocalTask<bool>, obj: &glib::Object| {
            unsafe {
                let t: *mut ffi::GTask = LocalTask::to_glib_none(t).0;

                assert_eq!(
                    ffi::g_task_is_valid(t as *mut ffi::GAsyncResult, obj.to_glib_none().0),
                    glib::ffi::GTRUE
                );
                ffi::g_task_return_boolean(t as *mut GTask, true.into_glib())
            };
        })
    );
}

// This is now broken since it expects a primitive bool
#[test]
fn test_return_value_gboolean() {
    #[cfg(not(feature = "v2_64"))]
    unsafe extern "C" fn value_free(value: *mut libc::c_void) {
        let _: glib::Value = from_glib_full(value as *mut glib::gobject_ffi::GValue);
    }

    assert_eq!(
        Ok(false),
        run_task(|t: &LocalTask<bool>, obj: &glib::Object| {
            unsafe {
                let value = glib::Value::from_type(glib::Type::BOOL);
                let value =
                    glib::translate::IntoGlibPtr::<*mut glib::gobject_ffi::GValue>::into_glib_ptr(
                        value,
                    );

                let t: *mut ffi::GTask = LocalTask::to_glib_none(t).0;
                assert_eq!(
                    ffi::g_task_is_valid(t as *mut ffi::GAsyncResult, obj.to_glib_none().0),
                    glib::ffi::GTRUE
                );

                glib::gobject_ffi::g_value_set_boolean(value, true.into_glib());

                #[cfg(feature = "v2_64")]
                ffi::g_task_return_value(t, value);
                #[cfg(not(feature = "v2_64"))]
                ffi::g_task_return_pointer(t, value as *mut c_void, Some(value_free));
            };
        })
    )
}

#[test]
fn test_return_value_from_rust() {
    assert_eq!(
        Ok(true),
        run_task(|t: &LocalTask<bool>, obj: &glib::Object| {
            t.clone().return_result(Ok(true));
        })
    );
}
