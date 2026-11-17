macro_rules! platforms {
    ($($($platform:expr);* => $module:ident),*) => {
        $(
            #[cfg(any($(target_os = $platform),*))]
            #[cfg_attr(not(any($(target_os = $platform),*)), allow(dead_code))]
            mod $module;

            #[cfg(any($(target_os = $platform),*))]
            pub use self::$module::*;

            // macos/freebsd/netbsd: libc 保留 ENOATTR，直接引用。
            #[cfg(all(any($(target_os = $platform),*), any(target_os = "macos", target_os = "freebsd", target_os = "netbsd")))]
            pub const ENOATTR: ::libc::c_int = ::libc::ENOATTR;

            // linux/android: 新版 libc crate 的 gnu/bionic 目标已不暴露
            // ENOATTR（连带 ENODATA 也已移除）。xattr 系统调用对"属性不存在"
            // 实际返回 ENODATA=61（内核 ABI 常量，与上游 xattr 1.x 经
            // rustix::Errno::NODATA 的取值一致），直接使用数值。
            #[cfg(all(any($(target_os = $platform),*), not(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))))]
            pub const ENOATTR: ::libc::c_int = 61;
        )*

        #[cfg(all(feature = "unsupported", not(any($($(target_os = $platform),*),*))))]
        #[cfg_attr(any($($(target_os = $platform),*),*), allow(dead_code))]
        mod unsupported;

        #[cfg(all(feature = "unsupported", not(any($($(target_os = $platform),*),*))))]
        pub use self::unsupported::*;
        #[cfg(all(feature = "unsupported", not(any($($(target_os = $platform),*),*))))]
        pub const ENOATTR: ::libc::c_int = 0;


        /// A constant indicating whether or not the target platform is supported.
        ///
        /// To make programmer's lives easier, this library builds on all platforms.
        /// However, all function calls on unsupported platforms will return
        /// `io::Error`s.
        ///
        /// Note: If you would like compilation to simply fail on unsupported platforms,
        /// turn of the `unsupported` feature.
        pub const SUPPORTED_PLATFORM: bool = cfg!(any($($(target_os = $platform),*),*));
    }
}

platforms! {
    "android"; "linux"; "macos" => linux_macos,
    "freebsd"; "netbsd" => bsd
}
