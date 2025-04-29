/// Declares the entry point of the configuration.
#[macro_export]
macro_rules! config {
    ($f:path) => {
        #[unsafe(no_mangle)]
        #[used]
        pub static mut JAY_CONFIG_ENTRY_V1: $crate::_private::ConfigEntry = {
            struct X;
            impl $crate::_private::Config for X {
                extern "C" fn configure() {
                    $f();
                }
            }
            $crate::_private::ConfigEntryGen::<X>::ENTRY
        };
    };
}

macro_rules! try_get {
    () => {{
        unsafe {
            let client = crate::_private::client::CLIENT.with(|client| client.get());
            if client.is_null() {
                None
            } else {
                Some(&*client)
            }
        }
    }};
}

macro_rules! get {
    () => {{ get!(Default::default()) }};
    ($def:expr) => {{
        let client = unsafe {
            let client = crate::_private::client::CLIENT.with(|client| client.get());
            if client.is_null() {
                return $def;
            }
            &*client
        };
        client
    }};
}

macro_rules! bitflags {
    (
        $(#[$attr1:meta])*
        $vis1:vis struct $name:ident($vis2:vis $rep:ty) {
            $(
                $(#[$attr2:meta])*
                $vis3:vis const $var:ident = $val:expr,
            )*
        }
    ) => {
        $(#[$attr1])*
        $vis1 struct $name($vis2 $rep);

        $(
            $(#[$attr2])*
            $vis3 const $var: $name = $name($val);
        )*

        impl std::ops::BitOr for $name {
            type Output = Self;

            fn bitor(self, rhs: Self) -> Self::Output {
                Self(self.0 | rhs.0)
            }
        }

        impl std::ops::BitAnd for $name {
            type Output = Self;

            fn bitand(self, rhs: Self) -> Self::Output {
                Self(self.0 & rhs.0)
            }
        }

        impl std::ops::BitOrAssign for $name {
            fn bitor_assign(&mut self, rhs: Self) {
                self.0 |= rhs.0;
            }
        }

        impl std::ops::BitAndAssign for $name {
            fn bitand_assign(&mut self, rhs: Self) {
                self.0 &= rhs.0;
            }
        }

        impl std::ops::BitXorAssign for $name {
            fn bitxor_assign(&mut self, rhs: Self) {
                self.0 ^= rhs.0;
            }
        }

        impl std::ops::Not for $name {
            type Output = Self;

            fn not(self) -> Self::Output {
                Self(!self.0)
            }
        }

        impl std::fmt::Debug for $name {
            #[allow(clippy::allow_attributes, clippy::bad_bit_mask, unused_mut)]
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let mut any = false;
                let mut v = self.0;
                $(
                    if $val != 0 && v & $val == $val {
                        if any {
                            write!(f, "|")?;
                        }
                        any = true;
                        write!(f, "{}", stringify!($var))?;
                        v &= !$val;
                    }
                )*
                if !any || v != 0 {
                    if any {
                        write!(f, "|")?;
                    }
                    write!(f, "0x{:x}", v)?;
                }
                Ok(())
            }
        }
    }
}
