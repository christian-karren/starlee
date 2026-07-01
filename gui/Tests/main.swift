import XCTest

// Entry point for the standalone test binary.
// XCTMain discovers all XCTestCase subclasses registered in the binary.

// We list all test suites explicitly so the runner knows what to execute.
var testSuites: [XCTestSuite] = [
    XCTestSuite(forTestCaseClass: StarleeClientTests.self),
    XCTestSuite(forTestCaseClass: MenuBarSettingsTests.self),
    XCTestSuite(forTestCaseClass: MenuBarIconTests.self),
    XCTestSuite(forTestCaseClass: StatusMenuControllerTests.self),
]

let topSuite = XCTestSuite(name: "StarleeGUITests")
for suite in testSuites {
    topSuite.addTest(suite)
}
topSuite.run()

let result = topSuite.testRun as! XCTestSuiteRun
let failCount = result.totalFailureCount
if failCount > 0 {
    exit(1)
}
