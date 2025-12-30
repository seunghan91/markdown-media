"""
MDM Chart to PNG Renderer
Converts chart data from document parsing to PNG images using matplotlib.

Supports: bar, line, pie, scatter, area, stacked bar, grouped bar, donut charts
"""

import json
import os
from dataclasses import dataclass, field
from enum import Enum
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple, Union
import matplotlib
matplotlib.use('Agg')  # Non-interactive backend for server use
import matplotlib.pyplot as plt
import matplotlib.font_manager as fm
import numpy as np


class ChartType(Enum):
    """Supported chart types."""
    BAR = "bar"
    LINE = "line"
    PIE = "pie"
    SCATTER = "scatter"
    AREA = "area"
    STACKED_BAR = "stacked_bar"
    GROUPED_BAR = "grouped_bar"
    DONUT = "donut"
    HORIZONTAL_BAR = "horizontal_bar"


@dataclass
class ChartStyle:
    """Chart styling configuration."""
    # Colors
    colors: List[str] = field(default_factory=lambda: [
        '#4E79A7', '#F28E2B', '#E15759', '#76B7B2', '#59A14F',
        '#EDC948', '#B07AA1', '#FF9DA7', '#9C755F', '#BAB0AC'
    ])
    background_color: str = "#FFFFFF"
    grid_color: str = "#E0E0E0"
    text_color: str = "#333333"

    # Typography
    title_font_size: int = 14
    label_font_size: int = 11
    tick_font_size: int = 10
    legend_font_size: int = 10
    font_family: str = "sans-serif"

    # Layout
    figure_width: float = 10.0
    figure_height: float = 6.0
    dpi: int = 150
    padding: float = 0.1

    # Grid
    show_grid: bool = True
    grid_alpha: float = 0.3
    grid_style: str = "--"

    # Legend
    show_legend: bool = True
    legend_position: str = "best"

    # Axes
    show_x_axis: bool = True
    show_y_axis: bool = True
    x_axis_rotation: int = 0

    @classmethod
    def dark_theme(cls) -> "ChartStyle":
        """Dark theme preset."""
        return cls(
            colors=['#60A5FA', '#F59E0B', '#EF4444', '#10B981', '#8B5CF6',
                    '#EC4899', '#06B6D4', '#F97316', '#84CC16', '#6366F1'],
            background_color="#1F2937",
            grid_color="#374151",
            text_color="#F3F4F6",
        )

    @classmethod
    def minimal_theme(cls) -> "ChartStyle":
        """Minimal, clean theme preset."""
        return cls(
            colors=['#2563EB', '#DC2626', '#059669', '#7C3AED', '#D97706'],
            background_color="#FFFFFF",
            grid_color="#F3F4F6",
            text_color="#1F2937",
            show_grid=False,
            grid_alpha=0.1,
        )

    @classmethod
    def presentation_theme(cls) -> "ChartStyle":
        """High contrast theme for presentations."""
        return cls(
            colors=['#1E40AF', '#DC2626', '#047857', '#7C2D12', '#4C1D95'],
            background_color="#FFFFFF",
            grid_color="#D1D5DB",
            text_color="#111827",
            title_font_size=18,
            label_font_size=14,
            tick_font_size=12,
            legend_font_size=12,
            figure_width=12.0,
            figure_height=7.0,
            dpi=200,
        )


@dataclass
class DataSeries:
    """A single data series for charts."""
    name: str
    values: List[float]
    color: Optional[str] = None
    marker: Optional[str] = None  # For line/scatter: 'o', 's', '^', etc.
    line_style: Optional[str] = None  # For line: '-', '--', '-.', ':'


@dataclass
class ChartData:
    """Complete chart data structure."""
    chart_type: ChartType
    title: str = ""
    x_label: str = ""
    y_label: str = ""
    categories: List[str] = field(default_factory=list)  # X-axis labels
    series: List[DataSeries] = field(default_factory=list)

    # For scatter plots with explicit x values
    x_values: Optional[List[float]] = None

    # For pie/donut charts
    explode: Optional[List[float]] = None
    start_angle: float = 90

    # Additional options
    stacked: bool = False
    show_values: bool = False
    value_format: str = "{:.1f}"

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "ChartData":
        """Create ChartData from dictionary (e.g., from JSON)."""
        chart_type_str = data.get("type", data.get("chart_type", "bar"))
        chart_type = ChartType(chart_type_str.lower())

        series_data = data.get("series", [])
        series = []
        for s in series_data:
            if isinstance(s, dict):
                series.append(DataSeries(
                    name=s.get("name", ""),
                    values=s.get("values", s.get("data", [])),
                    color=s.get("color"),
                    marker=s.get("marker"),
                    line_style=s.get("line_style", s.get("lineStyle")),
                ))
            elif isinstance(s, (list, tuple)):
                series.append(DataSeries(name="", values=list(s)))

        # Handle simple data format (values directly)
        if not series and "values" in data:
            values = data["values"]
            if isinstance(values[0], (list, tuple)):
                for i, v in enumerate(values):
                    series.append(DataSeries(name=f"Series {i+1}", values=list(v)))
            else:
                series.append(DataSeries(name="Data", values=values))

        # Handle labels as series names for pie charts
        if chart_type in (ChartType.PIE, ChartType.DONUT) and not series:
            labels = data.get("labels", data.get("categories", []))
            values = data.get("values", data.get("data", []))
            if labels and values:
                series = [DataSeries(name=str(l), values=[v]) for l, v in zip(labels, values)]

        return cls(
            chart_type=chart_type,
            title=data.get("title", ""),
            x_label=data.get("x_label", data.get("xLabel", data.get("xlabel", ""))),
            y_label=data.get("y_label", data.get("yLabel", data.get("ylabel", ""))),
            categories=data.get("categories", data.get("labels", [])),
            series=series,
            x_values=data.get("x_values", data.get("xValues")),
            explode=data.get("explode"),
            start_angle=data.get("start_angle", data.get("startAngle", 90)),
            stacked=data.get("stacked", False),
            show_values=data.get("show_values", data.get("showValues", False)),
            value_format=data.get("value_format", data.get("valueFormat", "{:.1f}")),
        )

    @classmethod
    def from_json(cls, json_str: str) -> "ChartData":
        """Create ChartData from JSON string."""
        return cls.from_dict(json.loads(json_str))

    @classmethod
    def from_rust_output(cls, data: Dict[str, Any]) -> "ChartData":
        """Create ChartData from Rust parser output format."""
        # Rust parser may use different field names
        chart_type_map = {
            "bar_chart": ChartType.BAR,
            "line_chart": ChartType.LINE,
            "pie_chart": ChartType.PIE,
            "scatter_chart": ChartType.SCATTER,
            "area_chart": ChartType.AREA,
        }

        raw_type = data.get("chart_type", data.get("type", "bar"))
        chart_type = chart_type_map.get(raw_type, ChartType(raw_type))

        return cls.from_dict({
            "chart_type": chart_type.value,
            "title": data.get("title", ""),
            "x_label": data.get("x_axis_label", ""),
            "y_label": data.get("y_axis_label", ""),
            "categories": data.get("x_categories", data.get("labels", [])),
            "series": data.get("data_series", data.get("series", [])),
            "show_values": data.get("show_data_labels", False),
        })


class ChartRenderer:
    """
    Renders chart data to PNG images using matplotlib.

    Supports multiple chart types with customizable styling.
    """

    def __init__(self, style: Optional[ChartStyle] = None):
        """
        Initialize the chart renderer.

        Args:
            style: Chart styling configuration. Uses default if not provided.
        """
        self.style = style or ChartStyle()
        self._setup_fonts()

    def _setup_fonts(self) -> None:
        """Configure fonts for Korean text support."""
        # Try to find Korean fonts
        korean_fonts = [
            'NanumGothic', 'Malgun Gothic', 'Apple SD Gothic Neo',
            'NanumBarunGothic', 'Noto Sans CJK KR', 'DejaVu Sans'
        ]

        available_fonts = [f.name for f in fm.fontManager.ttflist]

        for font in korean_fonts:
            if font in available_fonts:
                plt.rcParams['font.family'] = font
                break

        # Ensure minus sign displays correctly
        plt.rcParams['axes.unicode_minus'] = False

    def render(
        self,
        chart_data: Union[ChartData, Dict[str, Any]],
        output_path: str,
        style: Optional[ChartStyle] = None
    ) -> str:
        """
        Render chart data to PNG file.

        Args:
            chart_data: Chart data (ChartData object or dict)
            output_path: Path to save the PNG file
            style: Optional style override

        Returns:
            Path to the generated PNG file
        """
        if isinstance(chart_data, dict):
            chart_data = ChartData.from_dict(chart_data)

        style = style or self.style

        # Create figure with style
        fig, ax = plt.subplots(
            figsize=(style.figure_width, style.figure_height),
            dpi=style.dpi
        )
        fig.patch.set_facecolor(style.background_color)
        ax.set_facecolor(style.background_color)

        # Render based on chart type
        render_method = {
            ChartType.BAR: self._render_bar,
            ChartType.LINE: self._render_line,
            ChartType.PIE: self._render_pie,
            ChartType.SCATTER: self._render_scatter,
            ChartType.AREA: self._render_area,
            ChartType.STACKED_BAR: self._render_stacked_bar,
            ChartType.GROUPED_BAR: self._render_grouped_bar,
            ChartType.DONUT: self._render_donut,
            ChartType.HORIZONTAL_BAR: self._render_horizontal_bar,
        }.get(chart_data.chart_type, self._render_bar)

        render_method(ax, chart_data, style)

        # Apply common styling
        self._apply_styling(ax, chart_data, style)

        # Ensure output directory exists
        output_path = Path(output_path)
        output_path.parent.mkdir(parents=True, exist_ok=True)

        # Save figure
        plt.tight_layout(pad=style.padding * 10)
        plt.savefig(
            output_path,
            dpi=style.dpi,
            facecolor=style.background_color,
            edgecolor='none',
            bbox_inches='tight'
        )
        plt.close(fig)

        return str(output_path)

    def _get_colors(self, count: int, series: List[DataSeries], style: ChartStyle) -> List[str]:
        """Get colors for data series."""
        colors = []
        for i, s in enumerate(series[:count]):
            if s.color:
                colors.append(s.color)
            else:
                colors.append(style.colors[i % len(style.colors)])
        return colors

    def _render_bar(self, ax, data: ChartData, style: ChartStyle) -> None:
        """Render vertical bar chart."""
        if not data.series:
            return

        categories = data.categories or [str(i) for i in range(len(data.series[0].values))]
        x = np.arange(len(categories))

        if len(data.series) == 1:
            # Single series
            colors = self._get_colors(len(data.series[0].values),
                                      [DataSeries(name="", values=[v]) for v in data.series[0].values],
                                      style)
            bars = ax.bar(x, data.series[0].values, color=colors, edgecolor='white', linewidth=0.5)

            if data.show_values:
                for bar, val in zip(bars, data.series[0].values):
                    ax.text(bar.get_x() + bar.get_width()/2, bar.get_height(),
                           data.value_format.format(val),
                           ha='center', va='bottom',
                           fontsize=style.tick_font_size,
                           color=style.text_color)
        else:
            # Multiple series - grouped bars
            self._render_grouped_bar(ax, data, style)
            return

        ax.set_xticks(x)
        ax.set_xticklabels(categories, rotation=style.x_axis_rotation)

    def _render_grouped_bar(self, ax, data: ChartData, style: ChartStyle) -> None:
        """Render grouped bar chart."""
        if not data.series:
            return

        categories = data.categories or [str(i) for i in range(len(data.series[0].values))]
        x = np.arange(len(categories))
        n_series = len(data.series)
        width = 0.8 / n_series

        colors = self._get_colors(n_series, data.series, style)

        for i, (series, color) in enumerate(zip(data.series, colors)):
            offset = (i - n_series/2 + 0.5) * width
            bars = ax.bar(x + offset, series.values, width, label=series.name,
                         color=color, edgecolor='white', linewidth=0.5)

            if data.show_values:
                for bar, val in zip(bars, series.values):
                    ax.text(bar.get_x() + bar.get_width()/2, bar.get_height(),
                           data.value_format.format(val),
                           ha='center', va='bottom',
                           fontsize=style.tick_font_size - 1,
                           color=style.text_color)

        ax.set_xticks(x)
        ax.set_xticklabels(categories, rotation=style.x_axis_rotation)

    def _render_stacked_bar(self, ax, data: ChartData, style: ChartStyle) -> None:
        """Render stacked bar chart."""
        if not data.series:
            return

        categories = data.categories or [str(i) for i in range(len(data.series[0].values))]
        x = np.arange(len(categories))

        colors = self._get_colors(len(data.series), data.series, style)
        bottom = np.zeros(len(categories))

        for series, color in zip(data.series, colors):
            values = np.array(series.values)
            ax.bar(x, values, bottom=bottom, label=series.name,
                  color=color, edgecolor='white', linewidth=0.5)
            bottom += values

        ax.set_xticks(x)
        ax.set_xticklabels(categories, rotation=style.x_axis_rotation)

    def _render_horizontal_bar(self, ax, data: ChartData, style: ChartStyle) -> None:
        """Render horizontal bar chart."""
        if not data.series:
            return

        categories = data.categories or [str(i) for i in range(len(data.series[0].values))]
        y = np.arange(len(categories))

        if len(data.series) == 1:
            colors = self._get_colors(len(data.series[0].values),
                                      [DataSeries(name="", values=[v]) for v in data.series[0].values],
                                      style)
            bars = ax.barh(y, data.series[0].values, color=colors, edgecolor='white', linewidth=0.5)

            if data.show_values:
                for bar, val in zip(bars, data.series[0].values):
                    ax.text(bar.get_width(), bar.get_y() + bar.get_height()/2,
                           ' ' + data.value_format.format(val),
                           ha='left', va='center',
                           fontsize=style.tick_font_size,
                           color=style.text_color)
        else:
            # Multiple series - grouped horizontal bars
            n_series = len(data.series)
            height = 0.8 / n_series
            colors = self._get_colors(n_series, data.series, style)

            for i, (series, color) in enumerate(zip(data.series, colors)):
                offset = (i - n_series/2 + 0.5) * height
                ax.barh(y + offset, series.values, height, label=series.name,
                       color=color, edgecolor='white', linewidth=0.5)

        ax.set_yticks(y)
        ax.set_yticklabels(categories)

    def _render_line(self, ax, data: ChartData, style: ChartStyle) -> None:
        """Render line chart."""
        if not data.series:
            return

        x_vals = data.x_values
        if x_vals is None:
            x_vals = list(range(len(data.series[0].values)))
            if data.categories:
                x_vals = list(range(len(data.categories)))

        colors = self._get_colors(len(data.series), data.series, style)
        markers = ['o', 's', '^', 'D', 'v', '<', '>', 'p', 'h', '*']

        for i, (series, color) in enumerate(zip(data.series, colors)):
            marker = series.marker or markers[i % len(markers)]
            line_style = series.line_style or '-'

            ax.plot(x_vals[:len(series.values)], series.values,
                   marker=marker, linestyle=line_style,
                   color=color, label=series.name,
                   linewidth=2, markersize=6)

            if data.show_values:
                for x, y in zip(x_vals, series.values):
                    ax.annotate(data.value_format.format(y),
                              (x, y), textcoords="offset points",
                              xytext=(0, 10), ha='center',
                              fontsize=style.tick_font_size - 1,
                              color=style.text_color)

        if data.categories:
            ax.set_xticks(x_vals[:len(data.categories)])
            ax.set_xticklabels(data.categories, rotation=style.x_axis_rotation)

    def _render_area(self, ax, data: ChartData, style: ChartStyle) -> None:
        """Render area chart."""
        if not data.series:
            return

        x_vals = data.x_values
        if x_vals is None:
            x_vals = list(range(len(data.series[0].values)))

        colors = self._get_colors(len(data.series), data.series, style)

        if data.stacked:
            # Stacked area
            values_stack = [np.array(s.values) for s in data.series]
            ax.stackplot(x_vals, *values_stack,
                        labels=[s.name for s in data.series],
                        colors=colors, alpha=0.7)
        else:
            # Overlapping areas
            for series, color in zip(data.series, colors):
                ax.fill_between(x_vals[:len(series.values)], series.values,
                              alpha=0.5, color=color, label=series.name)
                ax.plot(x_vals[:len(series.values)], series.values,
                       color=color, linewidth=1.5)

        if data.categories:
            ax.set_xticks(x_vals[:len(data.categories)])
            ax.set_xticklabels(data.categories, rotation=style.x_axis_rotation)

    def _render_scatter(self, ax, data: ChartData, style: ChartStyle) -> None:
        """Render scatter plot."""
        if not data.series:
            return

        colors = self._get_colors(len(data.series), data.series, style)
        markers = ['o', 's', '^', 'D', 'v', '<', '>', 'p', 'h', '*']

        for i, (series, color) in enumerate(zip(data.series, colors)):
            marker = series.marker or markers[i % len(markers)]

            # X values can come from data.x_values or categories
            if data.x_values:
                x_vals = data.x_values[:len(series.values)]
            elif data.categories:
                x_vals = list(range(len(series.values)))
            else:
                x_vals = list(range(len(series.values)))

            ax.scatter(x_vals, series.values,
                      c=color, marker=marker, s=60,
                      label=series.name, alpha=0.7, edgecolors='white')

        if data.categories and not data.x_values:
            ax.set_xticks(list(range(len(data.categories))))
            ax.set_xticklabels(data.categories, rotation=style.x_axis_rotation)

    def _render_pie(self, ax, data: ChartData, style: ChartStyle) -> None:
        """Render pie chart."""
        if not data.series:
            return

        # For pie charts, flatten series into single list
        if len(data.series) == 1 and len(data.series[0].values) > 1:
            values = data.series[0].values
            labels = data.categories or [f"Item {i+1}" for i in range(len(values))]
        else:
            values = [s.values[0] if s.values else 0 for s in data.series]
            labels = [s.name for s in data.series]

        colors = self._get_colors(len(values), data.series, style)
        explode = data.explode or [0] * len(values)

        wedges, texts, autotexts = ax.pie(
            values, labels=labels, colors=colors,
            explode=explode, startangle=data.start_angle,
            autopct='%1.1f%%' if data.show_values else None,
            pctdistance=0.75,
            textprops={'fontsize': style.label_font_size, 'color': style.text_color}
        )

        # Style the percentage labels
        if data.show_values:
            for autotext in autotexts:
                autotext.set_fontsize(style.tick_font_size)
                autotext.set_color('white')
                autotext.set_fontweight('bold')

        ax.axis('equal')

    def _render_donut(self, ax, data: ChartData, style: ChartStyle) -> None:
        """Render donut chart."""
        if not data.series:
            return

        # Get values and labels
        if len(data.series) == 1 and len(data.series[0].values) > 1:
            values = data.series[0].values
            labels = data.categories or [f"Item {i+1}" for i in range(len(values))]
        else:
            values = [s.values[0] if s.values else 0 for s in data.series]
            labels = [s.name for s in data.series]

        colors = self._get_colors(len(values), data.series, style)
        explode = data.explode or [0] * len(values)

        wedges, texts, autotexts = ax.pie(
            values, labels=labels, colors=colors,
            explode=explode, startangle=data.start_angle,
            autopct='%1.1f%%' if data.show_values else None,
            pctdistance=0.75,
            wedgeprops=dict(width=0.5),  # Creates donut hole
            textprops={'fontsize': style.label_font_size, 'color': style.text_color}
        )

        # Style the percentage labels
        if data.show_values:
            for autotext in autotexts:
                autotext.set_fontsize(style.tick_font_size)
                autotext.set_color('white')
                autotext.set_fontweight('bold')

        # Add center text (total)
        total = sum(values)
        ax.text(0, 0, f'Total\n{data.value_format.format(total)}',
               ha='center', va='center',
               fontsize=style.label_font_size,
               color=style.text_color,
               fontweight='bold')

        ax.axis('equal')

    def _apply_styling(self, ax, data: ChartData, style: ChartStyle) -> None:
        """Apply common styling to the chart."""
        # Title
        if data.title:
            ax.set_title(data.title,
                        fontsize=style.title_font_size,
                        color=style.text_color,
                        fontweight='bold',
                        pad=15)

        # Axis labels
        if data.x_label:
            ax.set_xlabel(data.x_label,
                         fontsize=style.label_font_size,
                         color=style.text_color)

        if data.y_label:
            ax.set_ylabel(data.y_label,
                         fontsize=style.label_font_size,
                         color=style.text_color)

        # Grid (not for pie/donut)
        if style.show_grid and data.chart_type not in (ChartType.PIE, ChartType.DONUT):
            ax.grid(True, linestyle=style.grid_style,
                   alpha=style.grid_alpha, color=style.grid_color)
            ax.set_axisbelow(True)

        # Tick colors
        ax.tick_params(colors=style.text_color, labelsize=style.tick_font_size)

        # Spine colors
        for spine in ax.spines.values():
            spine.set_color(style.grid_color)

        # Legend (if multiple series and not pie)
        if (style.show_legend and
            len(data.series) > 1 and
            data.chart_type not in (ChartType.PIE, ChartType.DONUT) and
            any(s.name for s in data.series)):
            ax.legend(loc=style.legend_position,
                     fontsize=style.legend_font_size,
                     framealpha=0.9)

    def render_multiple(
        self,
        charts: List[Union[ChartData, Dict[str, Any]]],
        output_path: str,
        layout: Tuple[int, int] = None,
        style: Optional[ChartStyle] = None
    ) -> str:
        """
        Render multiple charts to a single PNG file.

        Args:
            charts: List of chart data
            output_path: Path to save the PNG file
            layout: Grid layout (rows, cols). Auto-calculated if None.
            style: Optional style override

        Returns:
            Path to the generated PNG file
        """
        style = style or self.style
        n_charts = len(charts)

        if layout is None:
            cols = min(2, n_charts)
            rows = (n_charts + cols - 1) // cols
            layout = (rows, cols)

        fig, axes = plt.subplots(
            layout[0], layout[1],
            figsize=(style.figure_width * layout[1], style.figure_height * layout[0]),
            dpi=style.dpi
        )
        fig.patch.set_facecolor(style.background_color)

        # Flatten axes for easy iteration
        if n_charts == 1:
            axes = [axes]
        else:
            axes = axes.flatten() if hasattr(axes, 'flatten') else [axes]

        for i, chart_data in enumerate(charts):
            if i >= len(axes):
                break

            ax = axes[i]
            ax.set_facecolor(style.background_color)

            if isinstance(chart_data, dict):
                chart_data = ChartData.from_dict(chart_data)

            render_method = {
                ChartType.BAR: self._render_bar,
                ChartType.LINE: self._render_line,
                ChartType.PIE: self._render_pie,
                ChartType.SCATTER: self._render_scatter,
                ChartType.AREA: self._render_area,
                ChartType.STACKED_BAR: self._render_stacked_bar,
                ChartType.GROUPED_BAR: self._render_grouped_bar,
                ChartType.DONUT: self._render_donut,
                ChartType.HORIZONTAL_BAR: self._render_horizontal_bar,
            }.get(chart_data.chart_type, self._render_bar)

            render_method(ax, chart_data, style)
            self._apply_styling(ax, chart_data, style)

        # Hide unused axes
        for i in range(n_charts, len(axes)):
            axes[i].set_visible(False)

        # Save
        output_path = Path(output_path)
        output_path.parent.mkdir(parents=True, exist_ok=True)

        plt.tight_layout(pad=2)
        plt.savefig(
            output_path,
            dpi=style.dpi,
            facecolor=style.background_color,
            edgecolor='none',
            bbox_inches='tight'
        )
        plt.close(fig)

        return str(output_path)


def render_chart(
    chart_data: Union[ChartData, Dict[str, Any], str],
    output_path: str,
    style: Optional[ChartStyle] = None,
    theme: Optional[str] = None
) -> str:
    """
    Convenience function to render a chart.

    Args:
        chart_data: Chart data (ChartData, dict, or JSON string)
        output_path: Path to save the PNG file
        style: Optional ChartStyle
        theme: Theme name ('dark', 'minimal', 'presentation')

    Returns:
        Path to the generated PNG file
    """
    if isinstance(chart_data, str):
        chart_data = json.loads(chart_data)

    if theme:
        theme_styles = {
            'dark': ChartStyle.dark_theme,
            'minimal': ChartStyle.minimal_theme,
            'presentation': ChartStyle.presentation_theme,
        }
        style = theme_styles.get(theme, ChartStyle)()

    renderer = ChartRenderer(style)
    return renderer.render(chart_data, output_path)


# CLI interface
if __name__ == "__main__":
    import argparse
    import sys

    parser = argparse.ArgumentParser(
        description="MDM Chart to PNG Renderer",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  %(prog)s chart_data.json output.png
  %(prog)s chart_data.json output.png --theme dark
  %(prog)s chart_data.json output.png --width 12 --height 8

Chart Data JSON Format:
  {
    "type": "bar",
    "title": "Sales Report",
    "categories": ["Q1", "Q2", "Q3", "Q4"],
    "series": [
      {"name": "2023", "values": [100, 120, 90, 150]},
      {"name": "2024", "values": [110, 130, 100, 160]}
    ],
    "x_label": "Quarter",
    "y_label": "Revenue ($K)",
    "show_values": true
  }

Supported chart types:
  bar, line, pie, scatter, area, stacked_bar, grouped_bar, donut, horizontal_bar
        """
    )

    parser.add_argument("input", help="Input JSON file with chart data")
    parser.add_argument("output", help="Output PNG file path")
    parser.add_argument("--theme", choices=['default', 'dark', 'minimal', 'presentation'],
                       default='default', help="Color theme")
    parser.add_argument("--width", type=float, default=10.0, help="Figure width in inches")
    parser.add_argument("--height", type=float, default=6.0, help="Figure height in inches")
    parser.add_argument("--dpi", type=int, default=150, help="Output resolution")
    parser.add_argument("--no-grid", action="store_true", help="Hide grid lines")
    parser.add_argument("--no-legend", action="store_true", help="Hide legend")

    args = parser.parse_args()

    # Load chart data
    try:
        with open(args.input, 'r', encoding='utf-8') as f:
            data = json.load(f)
    except FileNotFoundError:
        print(f"Error: File not found: {args.input}", file=sys.stderr)
        sys.exit(1)
    except json.JSONDecodeError as e:
        print(f"Error: Invalid JSON: {e}", file=sys.stderr)
        sys.exit(1)

    # Create style
    if args.theme == 'dark':
        style = ChartStyle.dark_theme()
    elif args.theme == 'minimal':
        style = ChartStyle.minimal_theme()
    elif args.theme == 'presentation':
        style = ChartStyle.presentation_theme()
    else:
        style = ChartStyle()

    style.figure_width = args.width
    style.figure_height = args.height
    style.dpi = args.dpi
    style.show_grid = not args.no_grid
    style.show_legend = not args.no_legend

    # Render
    try:
        output = render_chart(data, args.output, style=style)
        print(f"Chart rendered: {output}")
    except Exception as e:
        print(f"Error rendering chart: {e}", file=sys.stderr)
        sys.exit(1)
