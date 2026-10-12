// 在独立进程内只读核验最终安装包的 TIS 身份、路径与可用状态。
#import <AppKit/AppKit.h>
#import <Carbon/Carbon.h>

static NSString * const VakyuBundleID = @"app.qingjian.inputmethod";

static BOOL isInputMethodInstallURL(NSURL *url) {
    NSString *parent = url.URLByDeletingLastPathComponent.path;
    NSString *userRoot = [NSHomeDirectory() stringByAppendingPathComponent:@"Library/Input Methods"];
    return [parent isEqualToString:@"/Library/Input Methods"] || [parent isEqualToString:userRoot];
}

static BOOL checkSource(NSString *sourceID, NSURL *app, BOOL mode) {
    NSDictionary *filter = @{(__bridge NSString *)kTISPropertyInputSourceID: sourceID};
    NSArray *sources = CFBridgingRelease(TISCreateInputSourceList((__bridge CFDictionaryRef)filter, true));
    if (sources.count != 1) {
        fprintf(stderr, "输入源 %s 数量应为 1，实际为 %lu\n", sourceID.UTF8String, (unsigned long)sources.count);
        return NO;
    }
    TISInputSourceRef source = (__bridge TISInputSourceRef)sources.firstObject;
    NSString *bundleID = (__bridge NSString *)TISGetInputSourceProperty(source, kTISPropertyBundleID);
    NSNumber *enabled = (__bridge NSNumber *)TISGetInputSourceProperty(source, kTISPropertyInputSourceIsEnabled);
    NSNumber *selectable = (__bridge NSNumber *)TISGetInputSourceProperty(source, kTISPropertyInputSourceIsSelectCapable);
    if (![bundleID isEqualToString:VakyuBundleID] || !enabled.boolValue || (mode && !selectable.boolValue)) {
        fprintf(stderr, "输入源 %s 身份或启用/可选状态不正确\n", sourceID.UTF8String);
        return NO;
    }
    // 本包显式声明菜单图标。其实际 URL 同时确认 TIS 没有指向构建目录或另一安装副本。
    NSURL *icon = (__bridge NSURL *)TISGetInputSourceProperty(source, kTISPropertyIconImageURL);
    NSString *resources = [[app URLByAppendingPathComponent:@"Contents/Resources" isDirectory:YES] URLByResolvingSymlinksInPath].path;
    NSString *iconPath = icon.isFileURL ? icon.URLByResolvingSymlinksInPath.path : nil;
    if (![iconPath hasPrefix:[resources stringByAppendingString:@"/"]] ||
        ![[NSFileManager defaultManager] isReadableFileAtPath:iconPath]) {
        fprintf(stderr, "输入源 %s 未指向最终安装目录的图标资源\n", sourceID.UTF8String);
        return NO;
    }
    return YES;
}

int main(int argc, const char *argv[]) {
    @autoreleasepool {
        if (argc != 2) {
            fprintf(stderr, "用法：verify_input_source <已安装的 Vakyu.app>\n");
            return 64;
        }
        if (strcmp(argv[1], "--current-source") == 0) {
            TISInputSourceRef current = TISCopyCurrentKeyboardInputSource();
            if (!current) return 1;
            NSString *identifier = (__bridge NSString *)TISGetInputSourceProperty(current, kTISPropertyInputSourceID);
            BOOL valid = [identifier isKindOfClass:NSString.class] && identifier.length > 0;
            if (valid) printf("%s\n", identifier.UTF8String);
            CFRelease(current);
            return valid ? 0 : 1;
        }
        NSURL *app = [[NSURL fileURLWithPath:[NSString stringWithUTF8String:argv[1]] isDirectory:YES] URLByResolvingSymlinksInPath];
        NSBundle *bundle = [NSBundle bundleWithURL:app];
        NSDictionary *info = bundle.infoDictionary;
        NSString *parentID = info[@"TISInputSourceID"];
        NSArray *modes = info[@"ComponentInputModeDict"][@"tsVisibleInputModeOrderedArrayKey"];
        if (![bundle.bundleIdentifier isEqualToString:VakyuBundleID] ||
            ![parentID isEqualToString:VakyuBundleID] || modes.count != 1 ||
            ![modes.firstObject isKindOfClass:NSString.class]) {
            fprintf(stderr, "安装包输入源元数据无效\n");
            return 65;
        }
        BOOL valid = YES;
        // Launch Services 也会发现工作区构建产物；只有标准用户/系统输入法目录才算另一份安装。
        for (NSURL *candidate in [NSWorkspace.sharedWorkspace URLsForApplicationsWithBundleIdentifier:VakyuBundleID]) {
            NSURL *resolved = candidate.URLByResolvingSymlinksInPath;
            if (isInputMethodInstallURL(resolved) && ![resolved.path isEqualToString:app.path] &&
                [[NSFileManager defaultManager] fileExistsAtPath:resolved.path]) {
                fprintf(stderr, "仍有另一个同身份输入法包：%s\n", resolved.path.fileSystemRepresentation);
                valid = NO;
            }
        }
        valid = checkSource(parentID, app, NO) && valid;
        valid = checkSource(modes.firstObject, app, YES) && valid;
        if (!valid) return 1;
        printf("输入源独立进程核验通过：父项、模式、安装路径、启用及可选状态一致\n");
        return 0;
    }
}
