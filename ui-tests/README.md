# TinyLVT UI Integration Test Plan

This document outlines the comprehensive user stories that should be covered by UI integration tests for TinyLVT. The tests are organized by functional areas and prioritized by the implementation phases outlined in the UI specification.

## Test Environment

Our tests use:
- **API**: Test backend running on random port with test database
- **Frontend**: Trunk-served UI with hot reloading disabled
- **Browser**: Firefox via Selenium/fantoccini for UI automation
- **Test Data**: Isolated test database with controlled test scenarios

### Running Tests

**Important**: UI integration tests must be run sequentially due to a known bug with parallel execution. Always use:

```bash
cargo test -- --test-threads=1 --nocapture
```

Or for specific test patterns:
```bash
cargo test authentication -- --test-threads=1 --nocapture
```

The project is configured with `test-threads = 1` in `.cargo/config.toml` to enforce this by default.

> **Note**: The root cause of the parallel execution issue is unknown and debugging has been deferred. Tests pass reliably when run sequentially.

## User Stories & Test Cases

### Phase 1: Authentication & Basic Profile (MVP)

#### Story: User Account Management
**As a user, I want to create and manage my account so I can participate in the system.**

- [x] **US-001**: Create new account with email verification
  - Navigate to registration page
  - Fill registration form with valid credentials
  - Verify account creation success message
  - Check email verification requirement
  - *API Coverage*: `create_account`, `verify_email`

- [x] **US-002**: Login with valid credentials
  - Navigate to login page
  - Enter valid username/password
  - Verify successful login and redirect
  - Verify session persistence across page reloads
  - *API Coverage*: `login`, `login_check`

- [x] **US-003**: Login failure with invalid credentials
  - Attempt login with incorrect password
  - Verify error message display
  - Verify no redirect occurs
  - *API Coverage*: `login`

- [x] **US-004**: Password reset flow
  - Click forgot password link
  - Enter email address
  - Verify reset email sent message
  - Complete password reset process
  - Login with new password
  - *API Coverage*: `forgot_password`, `reset_password`

- [x] **US-005**: Email verification flow
  - Create account requiring verification
  - Access verification link
  - Verify account becomes verified
  - *API Coverage*: `verify_email`, `resend_verification_email`

- [x] **US-006**: View and edit profile
  - Access profile page when logged in
  - View current profile information
  - Edit display name
  - Verify changes persist
  - *API Coverage*: `user_profile`, `update_profile`

- [x] **US-007**: Logout functionality
  - Click logout from user menu
  - Verify session is terminated
  - Verify redirect to homepage
  - Verify protected pages require re-login
  - *API Coverage*: `logout`

#### Story: Basic Community Creation & Joining
**As a user, I want to create or join communities so I can participate in shared space allocation.**

- [x] **US-008**: Create new community
  - Navigate to communities page
  - Click create community button
  - Fill community creation form
  - Verify community is created with user as leader
  - *API Coverage*: `create_community`

- [x] **US-009**: View communities list
  - Access communities page
  - View list of user's communities
  - Click on community to access dashboard
  - *API Coverage*: `get_communities`

- [x] **US-010**: Join community via invite
  - Receive community invite
  - Click invite acceptance link
  - Verify community membership
  - Verify default active status
  - *API Coverage*: `accept_invite`, `get_invites`

- [x] **US-010b**: Accept community invite via direct link
  - Generate shareable invite link from community settings
  - Navigate directly to invite acceptance URL with query parameter
  - Verify automatic invite acceptance when authenticated
  - Verify redirect to communities page after successful acceptance
  - Verify community membership is established
  - *API Coverage*: `accept_invite`

### Phase 2: Core Community & Site Management

#### Story: Community Administration
**As a community moderator, I want to manage community settings and members so I can organize the community effectively.**

- [ ] **US-011**: Access community management page
  - Login as moderator+ user
  - Access community management interface
  - Verify management options are visible
  - *API Coverage*: `get_communities`

- [ ] **US-012**: Invite new community members
  - Access member management page
  - Create email-based invite
  - Create open invite link
  - Verify invite appears in pending list
  - *API Coverage*: `invite_member`

- [ ] **US-013**: Manage member roles and status (not planned for MVP)
  - View community members list
  - Edit member roles (within permission limits)
  - Toggle member active status
  - Verify changes are reflected immediately
  - *API Coverage*: `get_members`

- [ ] **US-014**: Configure membership schedule (not planned for MVP)
  - Access membership scheduling page
  - Upload CSV of membership periods
  - Set individual member schedules
  - Preview automatic activity changes
  - *API Coverage*: `set_membership_schedule`, `get_membership_schedule`

#### Story: Site & Space Creation
**As a community moderator, I want to create and configure sites and spaces so members can bid on them.**

- [ ] **US-015**: Create new site
  - Navigate to sites page for community
  - Click create site button
  - Fill site details form (name, description, auction params)
  - Configure possession period and lead times
  - Set timezone and open hours
  - Verify site creation success
  - *API Coverage*: `create_site`

- [ ] **US-016**: View and edit existing site
  - Access site from sites list
  - View site details page
  - Edit site configuration as moderator
  - Verify changes are saved
  - *API Coverage*: `get_site`, `update_site`

- [ ] **US-017**: Create spaces within site
  - Access site management page
  - Add new space to site
  - Configure space details (name, description, eligibility points)
  - Toggle space availability
  - *API Coverage*: `create_space`, `list_spaces`

- [ ] **US-018**: Edit and delete spaces
  - Edit existing space details
  - Update eligibility points
  - Delete unused space
  - Verify space list updates
  - *API Coverage*: `update_space`, `delete_space`

### Phase 3: Auction System & Bidding

#### Story: Auction Participation
**As a community member, I want to participate in auctions so I can gain access to spaces.**

- [ ] **US-019**: View active auctions
  - Navigate to auctions page
  - View list of current auctions
  - See countdown timers and round information
  - Filter auctions by community/site
  - *API Coverage*: `list_auctions`

- [ ] **US-020**: Place bids in active auction
  - Access auction details page
  - View current round status
  - Check eligibility for current round
  - Place bid on available space
  - Verify bid confirmation
  - *API Coverage*: `create_bid`, `get_eligibility`

- [ ] **US-021**: View auction results
  - Access completed auction
  - View final results by round
  - See space allocation outcomes
  - Check personal bid history
  - *API Coverage*: `list_auction_rounds`, `list_round_space_results_for_round`

- [ ] **US-022**: Monitor bid status during auction
  - Place bid and monitor status
  - Receive notifications when outbid
  - Track eligibility changes between rounds
  - View real-time auction updates
  - *API Coverage*: `get_bid`, `list_round_space_results_for_round`

#### Story: User Values & Proxy Bidding
**As a community member, I want to set my valuations and use proxy bidding so I can participate efficiently.**

- [ ] **US-023**: Set space valuations
  - Navigate to site values page
  - Set personal valuations for spaces
  - Update existing valuations
  - View valuation history
  - *API Coverage*: `create_or_update_user_value`, `list_user_values`

- [ ] **US-024**: Configure proxy bidding
  - Access auction proxy settings
  - Set maximum items to win
  - Configure bidding strategy
  - Enable/disable proxy bidding
  - *API Coverage*: `create_or_update_proxy_bidding`, `get_proxy_bidding`

- [ ] **US-025**: Proxy bidding execution
  - Enable proxy bidding for auction
  - Monitor automatic bid placement
  - Verify proxy bids respect user limits
  - Check proxy bidding results
  - *API Coverage*: `get_proxy_bidding`, `list_bids`

### Phase 4: Advanced Features & Error Handling

#### Story: Comprehensive User Experience
**As a user, I want the system to handle errors gracefully and provide helpful feedback.**

- [ ] **US-026**: Handle network errors gracefully
  - Simulate network interruptions during actions
  - Verify error messages are user-friendly
  - Test automatic retry mechanisms
  - Verify data consistency after reconnection

- [ ] **US-027**: Form validation and user feedback
  - Submit forms with invalid data
  - Verify client-side validation messages
  - Test field-specific error handling
  - Verify success messages for completed actions

- [ ] **US-028**: Permission-based feature visibility
  - Login as different role levels
  - Verify appropriate features are visible/hidden
  - Test permission enforcement on restricted actions
  - Verify graceful handling of permission errors

- [ ] **US-029**: Responsive design and mobile experience
  - Test on different screen sizes
  - Verify touch-friendly interfaces
  - Test mobile navigation patterns
  - Verify accessibility compliance

#### Story: Data Consistency & Real-time Updates
**As a user, I want to see consistent, up-to-date information across the system.**

- [ ] **US-030**: Real-time auction updates
  - Monitor auction page during active bidding
  - Verify bid updates appear immediately
  - Test countdown timer accuracy
  - Verify round transitions are smooth

- [ ] **US-031**: Cross-page data consistency
  - Make changes on one page
  - Navigate to related pages
  - Verify changes are reflected everywhere
  - Test browser refresh data persistence

## Test Implementation Structure

### Test Organization
```
ui-tests/src/
├── framework.rs          # Test environment setup and utilities
├── authentication.rs     # US-001 through US-007
├── community_management.rs # US-008 through US-014
├── site_management.rs    # US-015 through US-018
├── auction_system.rs     # US-019 through US-025
├── error_handling.rs     # US-026 through US-029
├── real_time.rs         # US-030 through US-031
└── helpers/
    ├── mod.rs
    ├── navigation.rs     # Common navigation helpers
    ├── forms.rs          # Form interaction utilities
    └── assertions.rs     # Custom assertion helpers
```

### Test Data Strategy
- Use the existing `test_helpers` crate for backend test data
- Create UI-specific test data factories for consistent scenarios
- Implement cleanup strategies to maintain test isolation
- Use deterministic data for reliable assertions

### Test Execution Strategy
- **Parallel Execution**: Tests in different modules can run in parallel
- **Sequential Within Module**: Related tests within a module run sequentially
- **Cleanup**: Each test cleans up its data to avoid interference
- **Retry Logic**: Implement retry for flaky UI interactions

## Success Criteria

### Coverage Goals
- **User Stories**: 100% of identified user stories have automated tests
- **API Endpoints**: 95% of APIClient methods are exercised by UI tests
- **User Roles**: All permission levels are tested for appropriate access
- **Error Scenarios**: Common error conditions are tested and handled

### Quality Metrics
- **Reliability**: Tests pass consistently (>95% success rate)
- **Speed**: Full test suite completes in under 10 minutes
- **Maintainability**: Tests are easy to update when UI changes
- **Coverage**: Critical user journeys have comprehensive test coverage

## Running Tests

### Local Development
```bash
# Run all UI tests
cargo test --package ui-tests

# Run specific test module
cargo test --package ui-tests authentication

# Run with debug output
RUST_LOG=ui_tests=debug,api=info cargo test --package ui-tests -- --nocapture

# Run specific user story
cargo test --package ui-tests test_user_account_creation -- --nocapture
```

### Continuous Integration
- Tests run on every pull request
- Separate test environments for different browser versions
- Parallel execution across multiple test runners
- Automatic retry for infrastructure-related failures

This comprehensive test plan ensures that all critical user journeys are validated through automated UI testing, providing confidence in the system's functionality from the user's perspective.

## Test Implementation Notes

When writing and maintaining UI integration tests, keep these principles in mind:

- **Match the Current Frontend Structure:**
  - Always ensure tests use the correct field IDs, selectors, and validation logic as implemented in the frontend. If the UI changes, update the tests accordingly.

- **Simulate Real User Interactions:**
  - Trigger all relevant frontend events (e.g., blur, change) as a user would. This ensures that validation and state updates occur as expected.

- **Check Actual UI Feedback:**
  - Assert for the specific user-facing messages, headings, or elements that indicate success or failure, not just generic alerts or backend responses.

- **Be Strict About Required Elements:**
  - If a field or UI element is required in the flow, the test should fail if it is missing. Do not silently skip or warn for missing required elements.

- **Read the Component Code When in Doubt:**
  - If unsure about the UI structure, consult the actual frontend component code to confirm field names, IDs, and flow before writing or updating tests.

These notes help ensure our UI tests remain robust, accurate, and maintainable as the application evolves.

# UI Tests

This crate contains UI integration tests for the application.

## Running Tests

Run the automated tests with:
```bash
cargo test
```

## Manual UI Testing

For manual testing and debugging, you can run the main function which will:
1. Set up the test environment (API server, frontend, and browser)
2. Create the Alice test user
3. Log Alice in automatically
4. Keep the browser open for manual inspection

To run the manual testing environment:
```bash
cargo run
```

The browser will open in **headed mode** (visible window) and stay open until you press Ctrl+C in the terminal. This allows you to interact with the browser and inspect the UI manually.

### Requirements

- Firefox browser installed (for geckodriver)
- geckodriver installed and in PATH
- trunk installed (`cargo install trunk`)

### Debug Output

The tracing setup uses the same configuration as the API server. For more verbose logging:
```bash
RUST_LOG=ui_tests=debug,api=info cargo run
```

Or for even more detailed output:
```bash
RUST_LOG=debug cargo run
```

# UI Integration Tests

This package contains UI integration tests that verify the frontend functionality by running real browser automation tests.

## Key Features

### Single Trunk Build per Test Session

The test framework ensures that `trunk build` only runs once per test session, even when multiple tests run in parallel. This significantly speeds up test execution and prevents conflicts.

**How it works:**
- The first test to call `TestEnvironment::setup()` triggers the trunk build
- All subsequent tests (including parallel ones) wait for the build to complete and reuse the result
- Uses `tokio::sync::OnceCell` for thread-safe, async-compatible synchronization

**To verify this behavior:**
1. Clean any previous build state: `cd ../ui && rm -rf dist/`
2. Run multiple tests in parallel with logging:
   ```bash
   RUST_LOG=ui_tests=info cargo test test_login -- --nocapture --test-threads=2
   ```
3. Look for the single "🔨 Building frontend with trunk build (first time only)" message in the logs
4. Subsequent test environments should show "♻️ Using cached trunk build result"

## Running Tests

### Single Test
```bash
# Run one test with verbose output
RUST_LOG=ui_tests=debug,api=info cargo test test_login_with_valid_credentials -- --nocapture
```

### Multiple Tests
```bash
# Run all authentication tests
cargo test authentication -- --test-threads=1

# Run tests in parallel (for when the bugs around parallel execution are resolved)
cargo test authentication -- --test-threads=4
```

### All Tests
```bash
# Run all UI tests
cargo test -- --test-threads=1
```

## Test Structure

Each test follows this pattern:
1. **Setup**: `TestEnvironment::setup()` starts API server, builds frontend (once), starts geckodriver and browser
2. **Execute**: Test-specific actions using browser automation
3. **Verify**: Assert expected outcomes
4. **Cleanup**: Automatic cleanup when `TestEnvironment` is dropped

## Architecture

- **TestEnvironment**: Manages the full test environment (API, frontend, browser)
- **Framework helpers**: Reusable functions like `login_user()`
- **Test modules**: Organized by feature (authentication, community, etc.)

## Dependencies

- **Backend**: Rust API server (from `../api`)
- **Frontend**: Trunk-built WASM frontend (from `../ui`) 
- **Browser**: Firefox via geckodriver (automatically managed)

## Debugging

### Frontend Not Loading
If tests fail with frontend connection issues:
1. Check that trunk is installed: `trunk --version`
2. Verify UI builds successfully: `cd ../ui && trunk build`
3. Check for port conflicts in test logs

### Browser Issues
- Tests run in headless Firefox by default
- For visual debugging, use `TestEnvironment::setup_headed()` in `main.rs`
- Geckodriver logs are suppressed but can be enabled in framework.rs

### Build Issues
If trunk build fails:
1. Check `../ui` directory exists and has proper Trunk.toml
2. Verify BACKEND_URL environment variable is set correctly
3. Check for wasm-pack and other trunk dependencies
