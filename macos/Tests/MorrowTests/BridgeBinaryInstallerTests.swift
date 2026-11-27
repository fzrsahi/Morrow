import Darwin
import XCTest
@testable import Morrow
import MorrowCore

final class BridgeBinaryInstallerTests: XCTestCase {
    private var scratch: String!
    private let fm = FileManager.default

    override func setUpWithError() throws {
        scratch = NSTemporaryDirectory() + "bridge-install-\(UUID().uuidString.prefix(8))"
        try fm.createDirectory(atPath: scratch, withIntermediateDirectories: true)
    }

    override func tearDownWithError() throws {
        try? fm.removeItem(atPath: scratch)
    }

    private func makeExecutableLayout() throws -> (execPath: String, execDir: String, helpersDir: String) {
        let contentsDir = scratch + "/Morrow.app/Contents"
        let execDir = contentsDir + "/MacOS"
        let helpersDir = contentsDir + "/Helpers"
        try fm.createDirectory(atPath: execDir, withIntermediateDirectories: true)
        try fm.createDirectory(atPath: helpersDir, withIntermediateDirectories: true)
        let execPath = execDir + "/Morrow"
        XCTAssertTrue(fm.createFile(atPath: execPath, contents: Data("app".utf8)))
        return (execPath, execDir, helpersDir)
    }

    private func mode(_ path: String) throws -> mode_t {
        try XCTUnwrap(SocketPath.lstat(path), path).st_mode & 0o777
    }

    private func isRegularFile(_ path: String) -> Bool {
        (try? fm.attributesOfItem(atPath: path))?[.type] as? FileAttributeType == .typeRegular
    }

    private func isSymlink(_ path: String) -> Bool {
        (try? fm.attributesOfItem(atPath: path))?[.type] as? FileAttributeType == .typeSymbolicLink
    }

    private func tmpEntries(in dir: String) throws -> [String] {
        try fm.contentsOfDirectory(atPath: dir).filter { $0.contains(".tmp.") }
    }

    func testStandardBundleLayoutReplacesExistingBridgeWithRegularFile() throws {
        let layout = try makeExecutableLayout()
        let realBridge = layout.helpersDir + "/morrow-bridge"
        let aliasBridge = layout.helpersDir + "/codeisland-bridge"
        XCTAssertTrue(fm.createFile(atPath: realBridge, contents: Data("NEW-MORROW-BRIDGE".utf8)))
        XCTAssertEqual(chmod(realBridge, 0o755), 0)
        try fm.createSymbolicLink(atPath: aliasBridge, withDestinationPath: "morrow-bridge")

        let destDir = scratch + "/dest"
        try fm.createDirectory(atPath: destDir, withIntermediateDirectories: true)
        let destPath = destDir + "/codeisland-bridge"
        XCTAssertTrue(fm.createFile(atPath: destPath, contents: Data("OLD-BRIDGE".utf8)))
        XCTAssertEqual(chmod(destPath, 0o755), 0)

        let ok = ConfigInstaller.installBridgeBinary(
            fm: fm,
            executablePath: layout.execPath,
            destinationPath: destPath
        )

        XCTAssertTrue(ok)
        XCTAssertTrue(isRegularFile(destPath))
        XCTAssertFalse(isSymlink(destPath))
        XCTAssertEqual(try String(contentsOfFile: destPath, encoding: .utf8), "NEW-MORROW-BRIDGE")
        XCTAssertEqual(try mode(destPath), 0o700)
        XCTAssertEqual(try tmpEntries(in: destDir), [])
    }

    func testRelativeSymlinkOnlyCodeislandBridgeReplacesExistingFile() throws {
        let layout = try makeExecutableLayout()
        let payload = layout.helpersDir + "/bridge-payload"
        let aliasBridge = layout.helpersDir + "/codeisland-bridge"
        XCTAssertTrue(fm.createFile(atPath: payload, contents: Data("SYMLINK-TARGET-BRIDGE".utf8)))
        try fm.createSymbolicLink(atPath: aliasBridge, withDestinationPath: "bridge-payload")

        let destDir = scratch + "/dest"
        try fm.createDirectory(atPath: destDir, withIntermediateDirectories: true)
        let destPath = destDir + "/codeisland-bridge"
        XCTAssertTrue(fm.createFile(atPath: destPath, contents: Data("OLD-BRIDGE".utf8)))
        XCTAssertEqual(chmod(destPath, 0o755), 0)

        let ok = ConfigInstaller.installBridgeBinary(
            fm: fm,
            executablePath: layout.execPath,
            destinationPath: destPath
        )

        XCTAssertTrue(ok)
        XCTAssertTrue(isRegularFile(destPath))
        XCTAssertFalse(isSymlink(destPath))
        XCTAssertEqual(try String(contentsOfFile: destPath, encoding: .utf8), "SYMLINK-TARGET-BRIDGE")
        XCTAssertEqual(try mode(destPath), 0o700)
        XCTAssertEqual(try tmpEntries(in: destDir), [])
    }

    func testRelativeSymlinkMorrowBridgeToCodeislandBridgeReplacesExistingFile() throws {
        let layout = try makeExecutableLayout()
        let realBridge = layout.helpersDir + "/codeisland-bridge"
        let aliasMorrow = layout.helpersDir + "/morrow-bridge"
        XCTAssertTrue(fm.createFile(atPath: realBridge, contents: Data("FALLBACK-REAL-BRIDGE".utf8)))
        try fm.createSymbolicLink(atPath: aliasMorrow, withDestinationPath: "codeisland-bridge")

        let destDir = scratch + "/dest"
        try fm.createDirectory(atPath: destDir, withIntermediateDirectories: true)
        let destPath = destDir + "/codeisland-bridge"
        XCTAssertTrue(fm.createFile(atPath: destPath, contents: Data("OLD-BRIDGE".utf8)))
        XCTAssertEqual(chmod(destPath, 0o755), 0)

        let ok = ConfigInstaller.installBridgeBinary(
            fm: fm,
            executablePath: layout.execPath,
            destinationPath: destPath
        )

        XCTAssertTrue(ok)
        XCTAssertTrue(isRegularFile(destPath))
        XCTAssertFalse(isSymlink(destPath))
        XCTAssertEqual(try String(contentsOfFile: destPath, encoding: .utf8), "FALLBACK-REAL-BRIDGE")
        XCTAssertEqual(try mode(destPath), 0o700)
        XCTAssertEqual(try tmpEntries(in: destDir), [])
    }

    func testStaleTempBrokenSymlinksAreRemovedAndUnrelatedFilesPreserved() throws {
        let layout = try makeExecutableLayout()
        let realBridge = layout.helpersDir + "/morrow-bridge"
        XCTAssertTrue(fm.createFile(atPath: realBridge, contents: Data("CLEAN-BRIDGE".utf8)))

        let destDir = scratch + "/dest"
        try fm.createDirectory(atPath: destDir, withIntermediateDirectories: true)
        let destPath = destDir + "/codeisland-bridge"
        let stale1 = destDir + "/codeisland-bridge.tmp.11111"
        let stale2 = destDir + "/codeisland-bridge.tmp.22222"
        let sessionsPath = destDir + "/sessions.json"
        try fm.createSymbolicLink(atPath: stale1, withDestinationPath: "morrow-bridge")
        try fm.createSymbolicLink(atPath: stale2, withDestinationPath: "morrow-bridge")
        XCTAssertTrue(fm.createFile(atPath: sessionsPath, contents: Data("{}".utf8)))

        let ok = ConfigInstaller.installBridgeBinary(
            fm: fm,
            executablePath: layout.execPath,
            destinationPath: destPath
        )

        XCTAssertTrue(ok)
        XCTAssertNil(SocketPath.lstat(stale1))
        XCTAssertNil(SocketPath.lstat(stale2))
        XCTAssertEqual(try String(contentsOfFile: sessionsPath, encoding: .utf8), "{}")
        XCTAssertEqual(try String(contentsOfFile: destPath, encoding: .utf8), "CLEAN-BRIDGE")
        XCTAssertEqual(try tmpEntries(in: destDir), [])
    }

    func testFreshInstallFromSwiftPMDevLayoutCreatesPrivateDestinationDir() throws {
        let execDir = scratch + "/dev-bin"
        try fm.createDirectory(atPath: execDir, withIntermediateDirectories: true)
        let execPath = execDir + "/Morrow"
        let devBridge = execDir + "/morrow-bridge"
        XCTAssertTrue(fm.createFile(atPath: execPath, contents: Data("app".utf8)))
        XCTAssertTrue(fm.createFile(atPath: devBridge, contents: Data("DEV-BRIDGE".utf8)))

        let destDir = scratch + "/new-dest"
        let destPath = destDir + "/codeisland-bridge"

        let ok = ConfigInstaller.installBridgeBinary(
            fm: fm,
            executablePath: execPath,
            destinationPath: destPath
        )

        XCTAssertTrue(ok)
        XCTAssertEqual(try mode(destDir), 0o700)
        XCTAssertEqual(try mode(destPath), 0o700)
        XCTAssertTrue(isRegularFile(destPath))
        XCTAssertEqual(try String(contentsOfFile: destPath, encoding: .utf8), "DEV-BRIDGE")
        XCTAssertEqual(try tmpEntries(in: destDir), [])
    }

    func testDanglingSymlinkSourceFailsSafelyWithoutClobberingDestination() throws {
        let layout = try makeExecutableLayout()
        let brokenSource = layout.helpersDir + "/codeisland-bridge"
        try fm.createSymbolicLink(atPath: brokenSource, withDestinationPath: "missing-binary")

        let destDir = scratch + "/dest"
        try fm.createDirectory(atPath: destDir, withIntermediateDirectories: true)
        let destPath = destDir + "/codeisland-bridge"
        XCTAssertTrue(fm.createFile(atPath: destPath, contents: Data("EXISTING-BRIDGE".utf8)))

        let ok = ConfigInstaller.installBridgeBinary(
            fm: fm,
            executablePath: layout.execPath,
            destinationPath: destPath
        )

        XCTAssertFalse(ok)
        XCTAssertEqual(try String(contentsOfFile: destPath, encoding: .utf8), "EXISTING-BRIDGE")
        XCTAssertEqual(try tmpEntries(in: destDir), [])
    }
}
