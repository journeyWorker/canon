## ADDED Requirements

### Requirement: A widget renders on demand

#### Scenario: A widget renders when requested
- **WHEN** a caller requests the widget
- **THEN** it renders successfully

#### Scenario: A widget handles an empty state
- **WHEN** the widget has no data
- **THEN** it renders a placeholder

#### Scenario: A widget reports a load error
- **WHEN** the widget's data source fails
- **THEN** it surfaces the error instead of a blank pane
