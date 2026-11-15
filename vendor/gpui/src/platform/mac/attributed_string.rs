use cocoa::base::id;
use cocoa::foundation::NSRange;
use objc::{class, msg_send, sel, sel_impl};

#[allow(non_snake_case)]
pub trait NSAttributedString: Sized {
    unsafe fn alloc(_: Self) -> id {
        msg_send![class!(NSAttributedString), alloc]
    }

    unsafe fn init_attributed_string(self, string: id) -> id;
    unsafe fn appendAttributedString_(self, attr_string: id);
    unsafe fn RTFDFromRange_documentAttributes_(self, range: NSRange, attrs: id) -> id;
    unsafe fn RTFFromRange_documentAttributes_(self, range: NSRange, attrs: id) -> id;
    unsafe fn string(self) -> id;
}

impl NSAttributedString for id {
    unsafe fn init_attributed_string(self, string: id) -> id {
        msg_send![self, initWithString: string]
    }

    unsafe fn appendAttributedString_(self, attr_string: id) {
        let _: () = msg_send![self, appendAttributedString: attr_string];
    }

    unsafe fn RTFDFromRange_documentAttributes_(self, range: NSRange, attrs: id) -> id {
        msg_send![self, RTFDFromRange: range documentAttributes: attrs]
    }

    unsafe fn RTFFromRange_documentAttributes_(self, range: NSRange, attrs: id) -> id {
        msg_send![self, RTFFromRange: range documentAttributes: attrs]
    }

    unsafe fn string(self) -> id {
        msg_send![self, string]
    }
}

pub trait NSMutableAttributedString: NSAttributedString {
    unsafe fn alloc(_: Self) -> id {
        msg_send![class!(NSMutableAttributedString), alloc]
    }
}

impl NSMutableAttributedString for id {}

#[cfg(test)]
mod tests {
    use super::*;
    use cocoa::appkit::NSImage;
    use cocoa::base::nil;
    use cocoa::foundation::NSString;
    #[test]
    #[ignore]
    fn test_nsattributed_string() {
        #[allow(non_snake_case)]
        pub trait NSTextAttachment: Sized {
            unsafe fn alloc(_: Self) -> id {
                msg_send![class!(NSTextAttachment), alloc]
            }
        }

        impl NSTextAttachment for id {}

        unsafe {
            let image: id = msg_send![class!(NSImage), alloc];
            image.initWithContentsOfFile_(NSString::alloc(nil).init_str("test.jpeg"));
            let _size = image.size();

            let string = NSString::alloc(nil).init_str("Test String");
            let attr_string = NSMutableAttributedString::alloc(nil).init_attributed_string(string);
            let hello_string = NSString::alloc(nil).init_str("Hello World");
            let hello_attr_string =
                NSAttributedString::alloc(nil).init_attributed_string(hello_string);
            attr_string.appendAttributedString_(hello_attr_string);

            let attachment = NSTextAttachment::alloc(nil);
            let _: () = msg_send![attachment, setImage: image];
            let image_attr_string =
                msg_send![class!(NSAttributedString), attributedStringWithAttachment: attachment];
            attr_string.appendAttributedString_(image_attr_string);

            let another_string = NSString::alloc(nil).init_str("Another String");
            let another_attr_string =
                NSAttributedString::alloc(nil).init_attributed_string(another_string);
            attr_string.appendAttributedString_(another_attr_string);

            let _len: cocoa::foundation::NSUInteger = msg_send![attr_string, length];


            let rtfd_data = attr_string.RTFDFromRange_documentAttributes_(
                NSRange::new(0, msg_send![attr_string, length]),
                nil,
            );
            assert_ne!(rtfd_data, nil);


        }
    }
}
