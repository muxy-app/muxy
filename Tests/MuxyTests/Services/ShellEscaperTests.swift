import Foundation
import Testing

@testable import Muxy

@Suite("ShellEscaper")
struct ShellEscaperTests {
    @Test("plain path is returned unchanged")
    func plainPath() {
        #expect(ShellEscaper.escape("/Users/alice/file.txt") == "/Users/alice/file.txt")
    }

    @Test("empty string is returned unchanged")
    func empty() {
        #expect(ShellEscaper.escape("") == "")
    }

    @Test("path with space is single-quoted")
    func withSpace() {
        #expect(ShellEscaper.escape("/tmp/my file.txt") == "'/tmp/my file.txt'")
    }

    @Test("path with parentheses is single-quoted")
    func withParens() {
        #expect(ShellEscaper.escape("/tmp/Dir (copy)/x") == "'/tmp/Dir (copy)/x'")
    }

    @Test("path with double quote is single-quoted")
    func withDoubleQuote() {
        #expect(ShellEscaper.escape("/tmp/\"x\".txt") == "'/tmp/\"x\".txt'")
    }

    @Test("path with backslash is single-quoted")
    func withBackslash() {
        #expect(ShellEscaper.escape("/tmp/a\\b") == "'/tmp/a\\b'")
    }

    @Test("path with shell metacharacters is single-quoted")
    func withShellMeta() {
        #expect(ShellEscaper.escape("/tmp/a$b") == "'/tmp/a$b'")
        #expect(ShellEscaper.escape("/tmp/a`b`") == "'/tmp/a`b`'")
        #expect(ShellEscaper.escape("/tmp/a!b") == "'/tmp/a!b'")
        #expect(ShellEscaper.escape("/tmp/a&b") == "'/tmp/a&b'")
        #expect(ShellEscaper.escape("/tmp/a|b") == "'/tmp/a|b'")
        #expect(ShellEscaper.escape("/tmp/a;b") == "'/tmp/a;b'")
    }

    @Test("single quote in path is escaped using close-escape-open pattern")
    func withSingleQuote() {
        #expect(ShellEscaper.escape("/tmp/it's.txt") == "'/tmp/it'\\''s.txt'")
    }

    @Test("multiple single quotes each get escaped")
    func withMultipleSingleQuotes() {
        #expect(ShellEscaper.escape("a'b'c") == "'a'\\''b'\\''c'")
    }

    @Test("path with space and single quote combines escapes")
    func withSpaceAndSingleQuote() {
        #expect(ShellEscaper.escape("it's my file") == "'it'\\''s my file'")
    }

    @Test("path with only alphanumerics and safe punctuation stays plain")
    func safePunctuation() {
        #expect(ShellEscaper.escape("/Users/a/b-c_d.1.txt") == "/Users/a/b-c_d.1.txt")
    }

    @Test("newline and control separators are quoted")
    func controlSeparators() {
        #expect(ShellEscaper.escape("a\nid") == "'a\nid'")
        #expect(ShellEscaper.escape("a\tb") == "'a\tb'")
        #expect(ShellEscaper.escape("a\rb") == "'a\rb'")
    }

    @Test("redirection, glob, and assignment characters are quoted")
    func redirectionAndGlobs() {
        #expect(ShellEscaper.escape("a>b") == "'a>b'")
        #expect(ShellEscaper.escape("*.swift") == "'*.swift'")
        #expect(ShellEscaper.escape("a=b") == "'a=b'")
    }

    @Test("newline in a remote path argument cannot break out")
    func newlinePathIsContained() {
        #expect(RemoteCommandBuilder.quoteRemotePath("~/a\nid") == "~/'a\nid'")
        #expect(RemoteCommandBuilder.quoteRemotePath("/proj/a\nid") == "'/proj/a\nid'")
    }

    @Test("tilde is quoted so escape never triggers remote home expansion")
    func tildeIsQuoted() {
        #expect(ShellEscaper.escape("~root") == "'~root'")
        #expect(ShellEscaper.escape("~/code") == "'~/code'")
    }

    @Test("quoteRemotePath expands only a leading tilde and quotes other tildes")
    func quoteRemotePathTildeHandling() {
        #expect(RemoteCommandBuilder.quoteRemotePath("~") == "~")
        #expect(RemoteCommandBuilder.quoteRemotePath("~/code") == "~/code")
        #expect(RemoteCommandBuilder.quoteRemotePath("~root") == "'~root'")
        #expect(RemoteCommandBuilder.quoteRemotePath("/a/~b") == "'/a/~b'")
    }
}
