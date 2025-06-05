# TinyLVT Frontend UI Specification

This document defines the complete frontend interface for TinyLVT, a community-based land value tax auction system. The UI enables communities to manage shared spaces through automated auction mechanisms.

## System Overview

TinyLVT is a space allocation system using continuous auctions with activity rules. Communities create sites with spaces that are regularly auctioned to members. The system uses eligibility points, proxy bidding, and scheduled membership to fairly distribute access to shared resources.

## Core Concepts

- **Communities**: Groups of users with different roles (member, moderator, coleader, leader)
- **Sites**: Physical locations containing one or more spaces
- **Spaces**: Individual units within a site that can be possessed
- **Auctions**: Time-bound events where spaces are allocated through bidding
- **Rounds**: Sequential phases within an auction with eligibility requirements
- **Eligibility Points**: Accumulated through participation, required for bidding
- **Proxy Bidding**: Automated bidding up to user-specified values

## Authentication & Account Management

### Login/Registration Pages
- **Route**: `/login`, `/register`
- **Features**:
  - Login form (username/password)
  - Account creation form (email, username, password)
  - Email verification flow
  - Password reset functionality
  - Session management with automatic logout

### Profile Management
- **Route**: `/profile`
- **Features**:
  - View/edit display name
  - View account balance
  - View email verification status
  - Change password
  - View membership history across communities

## Community Management

### Communities List
- **Route**: `/communities`
- **Features**:
  - List all communities user is member of
  - Show role for each community
  - Create new community button (for eligible users)
  - Join community via invite link

### Community Dashboard
- **Route**: `/community/:id`
- **Features**:
  - Community overview and statistics
  - Recent activity feed
  - Quick links to sites, members, settings
  - Role-based feature visibility

### Community Settings
- **Route**: `/community/:id/settings`
- **Access**: Moderator+
- **Features**:
  - Edit community name and description
  - Toggle new member default active status
  - View/manage community invites
  - Member management interface
  - Membership schedule configuration
  - Role assignment (coleader+ only)

### Member Management
- **Route**: `/community/:id/members`
- **Access**: Moderator+
- **Features**:
  - List all community members with roles and activity status
  - Invite new members (email or open invite)
  - Edit member roles (appropriate permission levels)
  - Toggle member active status
  - View member participation history

### Membership Scheduling
- **Route**: `/community/:id/schedule`
- **Access**: Moderator+
- **Features**:
  - Calendar view of membership schedule
  - Add/edit/remove schedule periods
  - Bulk import from CSV
  - Preview of automatic activity changes
  - Support for email hashing for privacy

## Site & Space Management

### Sites List
- **Route**: `/community/:id/sites`
- **Features**:
  - Grid/list view of all sites in community
  - Site thumbnails, names, descriptions
  - Quick stats (spaces, active auctions, next auction)
  - Create new site button (moderator+)

### Site Details
- **Route**: `/site/:id`
- **Features**:
  - Site information and description
  - List of spaces with availability status
  - Current and upcoming auctions
  - Open hours display
  - Site images
  - Edit button (moderator+)

### Site Management
- **Route**: `/site/:id/edit`
- **Access**: Moderator+
- **Features**:
  - Edit site name, description
  - Configure auction parameters:
    - Round duration
    - Bid increment
    - Activity rule eligibility progression
  - Set possession period
  - Configure lead times (auction, proxy bidding)
  - Open hours configuration (days/times)
  - Timezone setting
  - Auto-schedule toggle
  - Image upload/management

### Space Management
- **Route**: `/site/:id/spaces`
- **Access**: Moderator+
- **Features**:
  - List all spaces in site
  - Add/edit/delete spaces
  - Configure space eligibility points
  - Toggle space availability
  - Space descriptions and images
  - Preview auction eligibility impacts

## Auction System

### Active Auctions
- **Route**: `/auctions`
- **Features**:
  - List all active auctions user can participate in
  - Auction countdown timers
  - Current round information
  - User's current bids and eligibility
  - Quick bid placement

### Auction Details
- **Route**: `/auction/:id`
- **Features**:
  - Auction overview (site, possession period, timing)
  - Live round information with countdown
  - Space grid with current prices/winners
  - User's eligibility points for this auction
  - Bid history (user's own bids only)
  - Proxy bidding configuration

### Bidding Interface
- **Route**: `/auction/:id/bid`
- **Features**:
  - Current round status and time remaining
  - Space grid with real-time prices
  - Bid placement forms
  - Eligibility threshold indicator
  - Activity rule explanation
  - Bid confirmation dialogs
  - Real-time updates during active bidding

### Proxy Bidding
- **Route**: `/auction/:id/proxy`
- **Features**:
  - Configure maximum items to win
  - Set value preferences for each space
  - Preview proxy bidding strategy
  - Enable/disable proxy bidding
  - History of proxy bid executions

### User Values
- **Route**: `/site/:id/values`
- **Features**:
  - Set personal valuations for each space
  - Used for proxy bidding calculations
  - Historical value adjustments
  - Impact on proxy bidding preview

## Bidding & Results

### My Bids
- **Route**: `/bids`
- **Features**:
  - History of all user bids across auctions
  - Filter by site, auction, space
  - Bid outcomes (won/lost/outbid)
  - Upcoming possession periods
  - Financial summary

### Auction Results
- **Route**: `/auction/:id/results`
- **Features**:
  - Final results for completed auctions
  - Round-by-round progression
  - Space allocation outcomes
  - Participation statistics
  - Payment calculations

### Possession Calendar
- **Route**: `/calendar`
- **Features**:
  - Calendar view of user's current and future possessions
  - Integration with external calendars
  - Possession details and site information
  - Conflict resolution for overlapping possessions

## Administrative Features

### Community Analytics
- **Route**: `/community/:id/analytics`
- **Access**: Moderator+
- **Features**:
  - Member participation statistics
  - Auction performance metrics
  - Revenue and payment tracking
  - Usage patterns and trends
  - Export capabilities

### System Health
- **Route**: `/admin`
- **Access**: System administrators
- **Features**:
  - Scheduler status and logs
  - Database performance metrics
  - User activity monitoring
  - System configuration

## Navigation & Layout

### Header Navigation
- Community selector dropdown
- User menu (profile, logout)
- Notifications bell
- Active auction indicator

### Sidebar Navigation
- Dashboard
- Communities
- Active Auctions
- My Bids
- Calendar
- Profile

### Footer
- Links to help documentation
- System status
- Version information

## Real-time Features

### Live Updates
- Auction countdown timers
- Real-time bid updates during rounds
- Notification system for auction events
- Activity indicators for ongoing auctions

### Notifications
- Auction start/end alerts
- Bid confirmations and outbid warnings
- Membership status changes
- Payment due notifications
- Community announcements

## Mobile Responsiveness

All pages must be fully responsive with mobile-optimized:
- Touch-friendly bid placement
- Simplified navigation for small screens
- Optimized auction monitoring
- Quick access to active bids

## Error Handling

- Global error message display
- Form validation with helpful messages
- Network error recovery
- Graceful handling of permission errors
- Automatic retry for transient failures

## Performance Requirements

- Fast page loads with data caching
- Efficient real-time updates
- Optimized image loading
- Progressive enhancement for slow connections
- Offline functionality for viewing historical data

## Security Considerations

- Session management and automatic logout
- Role-based feature visibility
- Input sanitization and validation
- Secure image upload handling
- Protection of sensitive community data

## Implementation Priority

### Phase 1 (MVP)
1. Authentication and basic profile
2. Community creation and joining
3. Basic site and space management
4. Simple auction participation
5. Bid placement and viewing

### Phase 2 (Core Features)
1. Proxy bidding system
2. Advanced community management
3. Membership scheduling
4. Comprehensive auction details
5. Results and analytics

### Phase 3 (Advanced Features)
1. Real-time updates and notifications
2. Advanced analytics and reporting
3. Mobile optimization
4. Calendar integration
5. System administration tools

This specification provides a comprehensive blueprint for implementing the TinyLVT frontend, ensuring all backend functionality is properly exposed through an intuitive user interface. 