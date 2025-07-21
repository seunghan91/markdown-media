class HolidaysController < ApplicationController
  def index
    year = params[:year].to_i
    month = params[:month].to_i
    
    # 유효한 연도와 월인지 확인
    if year < 1900 || year > 2100 || month < 1 || month > 12
      render json: { error: 'Invalid year or month' }, status: :bad_request
      return
    end
    
    # 여기에 실제 holiday 데이터를 가져오는 로직 추가
    holidays = get_holidays_for_month(year, month)
    
    # 공휴일이 없어도 정상적인 응답 반환
    render json: { 
      year: year,
      month: month,
      holidays: holidays,
      count: holidays.size
    }, status: :ok
  end
  
  private
  
  def get_holidays_for_month(year, month)
    # 실제 holiday 데이터를 반환하는 로직
    # 공휴일이 없는 경우 빈 배열 반환
    # 예: 2025년 7월은 공휴일이 없을 수 있음
    []
  end
end