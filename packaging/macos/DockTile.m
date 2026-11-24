#import <AppKit/AppKit.h>

static NSString *const MuxyDockIconKey = @"MuxyDockIcon";

@interface MuxyDockTilePlugIn : NSObject <NSDockTilePlugIn>
@property(nonatomic, strong) NSDockTile *dockTile;
@property(nonatomic, strong) NSUserDefaults *defaults;
@end

@implementation MuxyDockTilePlugIn

- (void)setDockTile:(NSDockTile *)dockTile {
    NSDistributedNotificationCenter *center = [NSDistributedNotificationCenter defaultCenter];
    [center removeObserver:self];
    _dockTile = dockTile;
    if (dockTile == nil) {
        return;
    }
    NSURL *contents = [[[[NSBundle bundleForClass:[self class]] bundleURL]
        URLByDeletingLastPathComponent] URLByDeletingLastPathComponent];
    NSString *identifier = [[NSBundle bundleWithURL:[contents URLByDeletingLastPathComponent]] bundleIdentifier];
    if (identifier == nil) {
        return;
    }
    self.defaults = [[NSUserDefaults alloc] initWithSuiteName:identifier];
    [center addObserver:self
               selector:@selector(iconChanged:)
                   name:[identifier stringByAppendingString:@".DockIconChanged"]
                 object:nil];
    [self showIcon];
}

- (void)iconChanged:(NSNotification *)notification {
    dispatch_async(dispatch_get_main_queue(), ^{
        [self showIcon];
    });
}

- (void)showIcon {
    NSData *data = [self.defaults dataForKey:MuxyDockIconKey];
    NSImage *image = data == nil ? nil : [[NSImage alloc] initWithData:data];
    if (image == nil) {
        self.dockTile.contentView = nil;
    } else {
        NSSize size = self.dockTile.size;
        NSImageView *view = [[NSImageView alloc] initWithFrame:NSMakeRect(0, 0, size.width, size.height)];
        view.imageScaling = NSImageScaleProportionallyUpOrDown;
        view.image = image;
        self.dockTile.contentView = view;
    }
    [self.dockTile display];
}

- (void)dealloc {
    [[NSDistributedNotificationCenter defaultCenter] removeObserver:self];
}

@end
