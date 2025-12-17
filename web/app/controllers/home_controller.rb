# frozen_string_literal: true

class HomeController < ApplicationController
  def index
    @page_title = "MDM Web - URL/PDF to Markdown Converter"
    @page_description = "Convert any URL or PDF to clean Markdown format"

    render inertia: "Home", props: inertia_props
  end
end
