class Holiday < ApplicationRecord
  validates :date, presence: true
  validates :name, presence: true
  validates :country, presence: true
  
  # 특정 년월의 공휴일 조회
  scope :for_month, ->(year, month) {
    start_date = Date.new(year, month, 1)
    end_date = start_date.end_of_month
    where(date: start_date..end_date)
  }
  
  # 특정 국가의 공휴일 조회
  scope :for_country, ->(country = 'KR') {
    where(country: country)
  }
end