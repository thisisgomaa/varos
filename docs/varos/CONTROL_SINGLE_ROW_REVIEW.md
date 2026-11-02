# Control single-row sizing

2026-11-01. Owner requested that Control never wraps into another row. Hosted Control uses a non-wrapping row with compact spacing and narrower Stroke, Opacity and Transform fields. Horizontal overflow stays available in narrow windows; legacy shell layout is unchanged. Tools dimensions are unchanged.

102 panel-system tests passed (4 optional captures ignored), including equal Control height across narrow/wide docked and floating widths. Scoped UI/app clippy passed. Native build and final-app verification passed on com.varos.app build 20261101.11. Measured height is 34 logical points with and without selection; selected native capture fits controls and clipping action on one row. Home round-trip retains layout/documents. Owner acceptance pending. No commit or full CI claimed.
